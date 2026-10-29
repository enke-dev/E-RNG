//! SQLite storage: contexts (issuers with their settings), customers, position templates, layouts and
//! invoices. One file in the app data directory.
//!
//! The `invoices` table holds both kinds of document (`Kind`): invoices and offers, which are
//! written alike and numbered apart. Numbers are assigned when a document is created, never for
//! drafts, and a created one cannot be changed or deleted any more (it can be duplicated).

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use anyhow::{anyhow, bail, Context as _, Result};
use en16931::Date;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::context::{Context, ContextUsage, Disposition, BRANDS};
use crate::draft::{build_invoice, totals};
use crate::model::{
    CustomLayout, Customer, InvoiceDraft, InvoiceFiles, Kind, LayoutUsage, Linked, ListEntry,
    Position, PositionTemplate, Settings, Status, StoredFile, StoredInvoice,
};
use crate::numbering::{protect_new_tokens, Calendar, Given, Pattern};
use crate::rates::{self, Rate, RateDef, RateUsage};
use crate::units::{self, DraftRef, TemplateRef, Unit, UnitDef, UnitUsage};

pub struct Store(Mutex<Connection>);

/// Where the numbers of a context stand in the period of a day
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Sequence {
    /// What the next number is called
    pub next_number: String,
    /// Its counter
    pub counter: u32,
    /// The highest number given in the period
    pub highest: Option<String>,
    /// The counter the sequence continues from, while that is what decides the next number
    pub floor: Option<u32>,
}

/// What `PRAGMA user_version` says of a database this app has migrated; a backup of an older one is
/// migrated on restore, so no step of `migrate` may ever be dropped.
pub const SCHEMA_VERSION: i32 = 11;

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        bail!("the database was written by a newer version of the app");
    }
    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL);
             CREATE TABLE customers (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             CREATE TABLE invoices (
                 id INTEGER PRIMARY KEY,
                 number TEXT UNIQUE,
                 status TEXT NOT NULL,
                 draft TEXT NOT NULL,
                 total TEXT,
                 exported_at TEXT,
                 updated_at TEXT NOT NULL
             );
             PRAGMA user_version = 1;",
        )?;
    }
    if version < 2 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS templates (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             PRAGMA user_version = 2;",
        )?;
    }
    if version < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS layouts (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             PRAGMA user_version = 3;",
        )?;
    }
    if version < 4 {
        // a database that was stamped back by hand may have the column already
        let has_column: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('invoices') WHERE name = 'paid_at'",
            [],
            |r| r.get(0),
        )?;
        if !has_column {
            conn.execute_batch("ALTER TABLE invoices ADD COLUMN paid_at TEXT;")?;
        }
        conn.execute_batch("PRAGMA user_version = 4;")?;
    }
    if version < 5 {
        migrate_to_contexts(conn)?;
    }
    if version < 6 {
        // the originals of an invoice, see `InvoiceFiles`
        if !has_column(conn, "invoices", "files")? {
            conn.execute_batch("ALTER TABLE invoices ADD COLUMN files TEXT;")?;
        }
        conn.execute_batch("PRAGMA user_version = 6;")?;
    }
    if version < 7 {
        migrate_to_floors(conn)?;
    }
    if version < 8 {
        migrate_to_kinds(conn)?;
    }
    if version < 9 {
        migrate_to_links(conn)?;
    }
    if version < 10 {
        migrate_to_units(conn)?;
    }
    if version < 11 {
        migrate_to_rates(conn)?;
    }
    ensure_app_state(conn)
}

/// App-wide values that belong to no context (when the last backup was made). Created on every open
/// rather than by a step: it came with schema 11 before that was released, so databases that are
/// at 11 already have to get it as well.
fn ensure_app_state(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    Ok(())
}

/// A unit as a position names it: code and label
type UnitKey = (String, String);

/// A rate as a position names it: value and label as it picked them, none for the default
type RateKey = Option<(String, String)>;

/// What the drafts and saved positions that a context's units or rates reach call them (`K` is the
/// key of the one or the other), in the order of their ids: the context's own and the shared saved
/// positions, then its drafts. Created documents are not looked at, they keep what they have.
struct Held<K> {
    templates: Vec<(TemplateRef, K)>,
    drafts: Vec<(DraftRef, Vec<K>)>,
}

fn key_of(position: &Position) -> UnitKey {
    (position.unit_code().to_owned(), position.unit_label())
}

fn rate_key_of(position: &Position) -> RateKey {
    let rate = position.tax_rate.as_deref().and_then(rates::canonical)?;
    let label = position.tax_label.as_deref().unwrap_or_default().trim();
    Some((rate, label.to_owned()))
}

fn held<K>(conn: &Connection, context: i64, key: fn(&Position) -> K) -> Result<Held<K>> {
    let mut statement = conn.prepare(
        "SELECT id, json, context_id FROM templates
         WHERE context_id IS NULL OR context_id = ?1 ORDER BY id",
    )?;
    let rows = statement
        .query_map([context], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let templates = rows
        .into_iter()
        .map(|(id, json, context_id)| {
            let position: Position = serde_json::from_str(&json)?;
            Ok((
                TemplateRef {
                    id,
                    description: position.description.clone(),
                    context_id,
                },
                key(&position),
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut statement = conn.prepare(
        "SELECT id, kind, draft FROM invoices
         WHERE context_id = ?1 AND status = 'draft' ORDER BY id",
    )?;
    let rows = statement
        .query_map([context], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let drafts = rows
        .into_iter()
        .map(|(invoice_id, kind, json)| {
            let draft: InvoiceDraft = serde_json::from_str(&json)?;
            Ok((
                DraftRef {
                    invoice_id,
                    kind: Kind::from_column(&kind),
                    customer: draft.customer.name.clone(),
                },
                draft.positions.iter().map(key).collect(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Held { templates, drafts })
}

impl<K: PartialEq> Held<K> {
    /// The saved positions and drafts that name `key`
    fn reached(&self, key: &K) -> (Vec<TemplateRef>, Vec<DraftRef>) {
        (
            self.templates
                .iter()
                .filter(|(_, k)| k == key)
                .map(|(r, _)| r.clone())
                .collect(),
            self.drafts
                .iter()
                .filter(|(_, keys)| keys.contains(key))
                .map(|(r, _)| r.clone())
                .collect(),
        )
    }
}

impl Held<UnitKey> {
    fn usage_of(&self, key: &UnitKey) -> UnitUsage {
        let (templates, drafts) = self.reached(key);
        UnitUsage { templates, drafts }
    }
}

impl Held<RateKey> {
    /// `default`: whether the rate is the context's default, the caller knows that
    fn usage_of(&self, rate: &(String, String), default: bool) -> RateUsage {
        let (templates, drafts) = self.reached(&Some(rate.clone()));
        RateUsage {
            templates,
            drafts,
            default,
        }
    }
}

/// Points the position at another unit when it names `from`, returns whether it did. Only the two
/// fields are written, the rest of the stored position stays as it is.
fn retarget(position: &mut serde_json::Value, from: &UnitKey, to: &UnitKey) -> Result<bool> {
    let named: Position = serde_json::from_value(position.clone())?;
    if &key_of(&named) != from {
        return Ok(false);
    }
    position["unit"] = to.0.clone().into();
    position["unit_label"] = to.1.clone().into();
    Ok(true)
}

/// Points the position at another rate when it picked `from`, returns whether it did. Only the two
/// fields are written.
fn retarget_rate(
    position: &mut serde_json::Value,
    from: &(String, String),
    to: &(String, String),
) -> Result<bool> {
    let named: Position = serde_json::from_value(position.clone())?;
    if rate_key_of(&named).as_ref() != Some(from) {
        return Ok(false);
    }
    position["tax_rate"] = to.0.clone().into();
    position["tax_label"] = to.1.clone().into();
    Ok(true)
}

/// Drafts of the context and saved positions it sees (shared ones included) that `retarget` takes
/// for its own and changes, in the caller's transaction. Nothing else about them changes, not even
/// the time they were saved: the lists keep their order.
fn rewrite_positions(
    conn: &Connection,
    context: i64,
    retarget: &dyn Fn(&mut serde_json::Value) -> Result<bool>,
) -> Result<()> {
    let templates = conn
        .prepare("SELECT id, json FROM templates WHERE context_id IS NULL OR context_id = ?1")?
        .query_map([context], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, json) in templates {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        if retarget(&mut value)? {
            conn.execute(
                "UPDATE templates SET json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&value)?, id],
            )?;
        }
    }
    let drafts = conn
        .prepare("SELECT id, draft FROM invoices WHERE context_id = ?1 AND status = 'draft'")?
        .query_map([context], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, json) in drafts {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let changed = value["positions"].as_array_mut().map_or(Ok(false), |all| {
            all.iter_mut().try_fold(false, |changed, position| {
                retarget(position).map(|hit| hit || changed)
            })
        })?;
        if changed {
            conn.execute(
                "UPDATE invoices SET draft = ?1 WHERE id = ?2",
                params![serde_json::to_string(&value)?, id],
            )?;
        }
    }
    Ok(())
}

/// Appends units to the list of a context, skipping a label the list has already (case aside)
fn append_units(conn: &Connection, context: i64, wanted: &[UnitDef]) -> Result<()> {
    let mut sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort) + 1, 0) FROM units WHERE context_id = ?1",
        [context],
        |r| r.get(0),
    )?;
    let mut labels: Vec<String> = conn
        .prepare("SELECT label FROM units WHERE context_id = ?1")?
        .query_map([context], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .map(|l| l.trim().to_lowercase())
        .collect();
    for unit in wanted {
        let label = unit.label.trim().to_lowercase();
        if labels.contains(&label) {
            continue;
        }
        conn.execute(
            "INSERT INTO units (context_id, code, label, sort) VALUES (?1, ?2, ?3, ?4)",
            params![context, unit.code, unit.label, sort],
        )?;
        labels.push(label);
        sort += 1;
    }
    Ok(())
}

/// Units are a list per context. Each context gets the defaults, nothing else is looked at: a
/// position whose unit is not in the list shows as such and can be changed. A context that has
/// units already (a database that was stamped back) is left alone.
fn migrate_to_units(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS units (
             id INTEGER PRIMARY KEY,
             context_id INTEGER NOT NULL,
             code TEXT NOT NULL,
             label TEXT NOT NULL,
             sort INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS units_context ON units (context_id);",
    )?;
    let contexts = tx
        .prepare("SELECT id FROM contexts ORDER BY id")?
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for context in contexts {
        let present: i64 = tx.query_row(
            "SELECT COUNT(*) FROM units WHERE context_id = ?1",
            [context],
            |r| r.get(0),
        )?;
        if present == 0 {
            append_units(&tx, context, &units::defaults())?;
        }
    }
    tx.execute_batch("PRAGMA user_version = 10;")?;
    tx.commit()?;
    Ok(())
}

/// Appends rates to the list of a context, skipping one whose label or rate the list has already
/// (case aside, `19,0` is `19`). What is wanted is stored trimmed with the rate in its canonical
/// form, a rate that is none is skipped.
fn append_rates(conn: &Connection, context: i64, wanted: &[RateDef]) -> Result<()> {
    let mut sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort) + 1, 0) FROM rates WHERE context_id = ?1",
        [context],
        |r| r.get(0),
    )?;
    let (mut labels, mut values): (Vec<String>, Vec<String>) = rates_in(conn, context)?
        .into_iter()
        .map(|r| {
            (
                r.label.trim().to_lowercase(),
                rates::canonical(&r.rate).unwrap_or(r.rate),
            )
        })
        .unzip();
    for def in wanted {
        let (label, Some(value)) = (def.label.trim(), rates::canonical(&def.rate)) else {
            continue;
        };
        if labels.contains(&label.to_lowercase()) || values.contains(&value) {
            continue;
        }
        conn.execute(
            "INSERT INTO rates (context_id, label, rate, sort) VALUES (?1, ?2, ?3, ?4)",
            params![context, label, value, sort],
        )?;
        labels.push(label.to_lowercase());
        values.push(value);
        sort += 1;
    }
    Ok(())
}

/// The rate a context charges as a list entry, for a list that lacks it: a name nothing in the
/// list has yet
fn charged_entry(list: &[Rate], rate: &str) -> RateDef {
    let taken = |label: &str| {
        list.iter()
            .any(|r| r.label.trim().to_lowercase() == label.to_lowercase())
    };
    let label = if taken(rates::CHARGED_LABEL) {
        format!("{} {rate} %", rates::CHARGED_LABEL)
    } else {
        rates::CHARGED_LABEL.to_owned()
    };
    RateDef {
        label,
        rate: rate.to_owned(),
    }
}

/// VAT rates are a list per context. Each context gets the defaults and the rate it charges now,
/// when that is none of them, called `Steuersatz`; nothing else is looked at, a position has no
/// rate of its own yet. A context that has rates already (a database that was stamped back) is
/// left alone.
fn migrate_to_rates(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS rates (
             id INTEGER PRIMARY KEY,
             context_id INTEGER NOT NULL,
             label TEXT NOT NULL,
             rate TEXT NOT NULL,
             sort INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS rates_context ON rates (context_id);",
    )?;
    let contexts = tx
        .prepare("SELECT id, json FROM contexts ORDER BY id")?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (context, json) in contexts {
        let present: i64 = tx.query_row(
            "SELECT COUNT(*) FROM rates WHERE context_id = ?1",
            [context],
            |r| r.get(0),
        )?;
        if present == 0 {
            let body: ContextBody = serde_json::from_str(&json)?;
            append_rates(&tx, context, &rates::defaults())?;
            // a rate that is none (blank, or one that never worked) is skipped by the append
            let charged = charged_entry(&rates_in(&tx, context)?, body.settings.tax_rate.trim());
            append_rates(&tx, context, &[charged])?;
        }
    }
    tx.execute_batch("PRAGMA user_version = 11;")?;
    tx.commit()?;
    Ok(())
}

/// Offers and invoices can be linked, many to many. Foreign keys are not enabled in this app, so
/// the store checks the kinds and the shared context and removes the links of a deleted draft.
fn migrate_to_links(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS document_links (
             offer_id INTEGER NOT NULL,
             invoice_id INTEGER NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (offer_id, invoice_id)
         );
         CREATE INDEX IF NOT EXISTS document_links_invoice ON document_links (invoice_id);
         PRAGMA user_version = 9;",
    )?;
    tx.commit()?;
    Ok(())
}

/// Invoices get a kind, offers are the second one. A number is unique per context and kind, which
/// SQLite can only change by building the table again.
fn migrate_to_kinds(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    if !has_column(&tx, "invoices", "kind")? {
        tx.execute_batch(
            "CREATE TABLE invoices_new (
                 id INTEGER PRIMARY KEY,
                 context_id INTEGER NOT NULL,
                 kind TEXT NOT NULL DEFAULT 'invoice',
                 number TEXT,
                 status TEXT NOT NULL,
                 draft TEXT NOT NULL,
                 total TEXT,
                 exported_at TEXT,
                 paid_at TEXT,
                 updated_at TEXT NOT NULL,
                 archived_at TEXT,
                 files TEXT,
                 counter INTEGER,
                 issued_on TEXT,
                 UNIQUE (context_id, kind, number)
             );
             INSERT INTO invoices_new
                 (id, context_id, number, status, draft, total, exported_at, paid_at, updated_at,
                  archived_at, files, counter, issued_on)
                 SELECT id, context_id, number, status, draft, total, exported_at, paid_at,
                     updated_at, archived_at, files, counter, issued_on
                 FROM invoices;
             DROP TABLE invoices;
             ALTER TABLE invoices_new RENAME TO invoices;",
        )?;
    }
    tx.execute_batch("PRAGMA user_version = 8;")?;
    tx.commit()?;
    Ok(())
}

/// Number floors (where a sequence continues from, see `Store::set_counter`) get a table, and a
/// document keeps the counter of its number and the day it was issued on, which the period of the
/// counter is worked out from. The letters `G`, `g` and `Q` became tokens of the pattern: bracketed
/// in every stored pattern, they keep writing what they wrote.
fn migrate_to_floors(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (column, kind) in [("counter", "INTEGER"), ("issued_on", "TEXT")] {
        if !has_column(&tx, "invoices", column)? {
            tx.execute_batch(&format!("ALTER TABLE invoices ADD COLUMN {column} {kind};"))?;
        }
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS number_floors (
             context_id INTEGER NOT NULL,
             kind TEXT NOT NULL DEFAULT 'invoice',
             period TEXT NOT NULL,
             next INTEGER NOT NULL,
             set_at TEXT NOT NULL,
             PRIMARY KEY (context_id, kind, period)
         );",
    )?;
    let rows = {
        let mut statement = tx.prepare("SELECT id, json FROM contexts")?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (id, json) in rows {
        // as a value, so nothing else of the stored settings is touched
        let mut body: serde_json::Value = serde_json::from_str(&json)?;
        let Some(pattern) = body["settings"]["number_pattern"]
            .as_str()
            .map(str::to_owned)
        else {
            continue;
        };
        let protected = protect_new_tokens(&pattern);
        if protected != pattern {
            body["settings"]["number_pattern"] = protected.into();
            tx.execute(
                "UPDATE contexts SET json = ?1 WHERE id = ?2",
                params![serde_json::to_string(&body)?, id],
            )?;
        }
    }
    tx.execute_batch("PRAGMA user_version = 7;")?;
    tx.commit()?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
        [table, column],
        |r| r.get(0),
    )?)
}

/// What a context keeps in its `json` column, the rest of it has columns
#[derive(serde::Serialize, serde::Deserialize)]
struct ContextBody {
    brand: String,
    settings: Settings,
}

/// The one settings row becomes the first context, every invoice belongs to it. Invoice numbers are
/// unique per context from here on, which SQLite can only change by building the table again.
fn migrate_to_contexts(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS contexts (
             id INTEGER PRIMARY KEY, json TEXT NOT NULL, archived_at TEXT
         );",
    )?;
    let has_settings = tx.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'settings'",
        [],
        |r| r.get::<_, bool>(0),
    )?;
    let settings_json: Option<String> = if has_settings {
        tx.query_row("SELECT json FROM settings WHERE id = 1", [], |r| r.get(0))
            .optional()?
    } else {
        None
    };
    let has_data = tx.query_row(
        "SELECT (SELECT COUNT(*) FROM invoices) + (SELECT COUNT(*) FROM customers) > 0",
        [],
        |r| r.get::<_, bool>(0),
    )?;
    // a fresh database has no context, the app asks for one
    if settings_json.is_some() || has_data {
        let settings = match settings_json {
            Some(json) => serde_json::from_str::<Settings>(&json)?.normalized(),
            None => Settings::default(),
        };
        let body = ContextBody {
            brand: BRANDS[0].into(),
            settings,
        };
        tx.execute(
            "INSERT OR IGNORE INTO contexts (id, json) VALUES (1, ?1)",
            [serde_json::to_string(&body)?],
        )?;
    }
    tx.execute_batch("DROP TABLE IF EXISTS settings;")?;

    if !has_column(&tx, "invoices", "context_id")? {
        let archived = if has_column(&tx, "invoices", "archived_at")? {
            "archived_at"
        } else {
            "NULL"
        };
        tx.execute_batch(&format!(
            "CREATE TABLE invoices_new (
                 id INTEGER PRIMARY KEY,
                 context_id INTEGER NOT NULL,
                 number TEXT,
                 status TEXT NOT NULL,
                 draft TEXT NOT NULL,
                 total TEXT,
                 exported_at TEXT,
                 paid_at TEXT,
                 updated_at TEXT NOT NULL,
                 archived_at TEXT,
                 UNIQUE (context_id, number)
             );
             INSERT INTO invoices_new
                 SELECT id, 1, number, status, draft, total, exported_at, paid_at, updated_at, {archived}
                 FROM invoices;
             DROP TABLE invoices;
             ALTER TABLE invoices_new RENAME TO invoices;"
        ))?;
    }
    for table in ["customers", "templates", "layouts"] {
        if !has_column(&tx, table, "context_id")? {
            tx.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN context_id INTEGER;"
            ))?;
        }
    }
    tx.execute_batch("PRAGMA user_version = 5;")?;
    tx.commit()?;
    Ok(())
}

fn read_unit(row: &Row) -> rusqlite::Result<Unit> {
    Ok(Unit {
        id: Some(row.get("id")?),
        context_id: row.get("context_id")?,
        code: row.get("code")?,
        label: row.get("label")?,
    })
}

fn units_in(conn: &Connection, context: i64) -> Result<Vec<Unit>> {
    let mut statement = conn.prepare(
        "SELECT id, context_id, code, label FROM units WHERE context_id = ?1 ORDER BY sort, id",
    )?;
    let rows = statement.query_map([context], read_unit)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn unit_row(conn: &Connection, id: i64) -> Result<Unit> {
    conn.query_row(
        "SELECT id, context_id, code, label FROM units WHERE id = ?1",
        [id],
        read_unit,
    )
    .optional()?
    .ok_or_else(|| anyhow!("unknown unit"))
}

/// A unit def as a unit of a list, for the checks
fn placed(context: i64, unit: &UnitDef) -> Unit {
    Unit {
        id: None,
        context_id: context,
        code: unit.code.clone(),
        label: unit.label.clone(),
    }
}

fn read_rate(row: &Row) -> rusqlite::Result<Rate> {
    Ok(Rate {
        id: Some(row.get("id")?),
        context_id: row.get("context_id")?,
        label: row.get("label")?,
        rate: row.get("rate")?,
    })
}

fn rates_in(conn: &Connection, context: i64) -> Result<Vec<Rate>> {
    let mut statement = conn.prepare(
        "SELECT id, context_id, label, rate FROM rates WHERE context_id = ?1 ORDER BY sort, id",
    )?;
    let rows = statement.query_map([context], read_rate)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn rate_row(conn: &Connection, id: i64) -> Result<Rate> {
    conn.query_row(
        "SELECT id, context_id, label, rate FROM rates WHERE id = ?1",
        [id],
        read_rate,
    )
    .optional()?
    .ok_or_else(|| anyhow!("unknown rate"))
}

/// A rate def as a rate of a list, for the checks
fn placed_rate(context: i64, rate: &RateDef) -> Rate {
    Rate {
        id: None,
        context_id: context,
        label: rate.label.clone(),
        rate: rate.rate.clone(),
    }
}

/// A rate of a list as a position names it
fn rate_key(rate: &Rate) -> (String, String) {
    (
        rates::canonical(&rate.rate).unwrap_or_else(|| rate.rate.clone()),
        rate.label.trim().to_owned(),
    )
}

/// The rate the context's positions take when they pick none, none for a context without VAT
fn default_rate(conn: &Connection, context: i64) -> Result<Option<String>> {
    let json: String = conn
        .query_row("SELECT json FROM contexts WHERE id = ?1", [context], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| anyhow!("unknown context"))?;
    let body: ContextBody = serde_json::from_str(&json)?;
    Ok(rates::canonical(&body.settings.tax_rate))
}

/// Writes one field of a context's settings. Only that field is written, the rest of the stored
/// settings stays as it is.
fn set_setting(
    conn: &Connection,
    context: i64,
    field: &str,
    value: serde_json::Value,
) -> Result<()> {
    let json: String = conn
        .query_row("SELECT json FROM contexts WHERE id = ?1", [context], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| anyhow!("unknown context"))?;
    let mut body: serde_json::Value = serde_json::from_str(&json)?;
    body["settings"][field] = value;
    conn.execute(
        "UPDATE contexts SET json = ?1 WHERE id = ?2",
        params![serde_json::to_string(&body)?, context],
    )?;
    Ok(())
}

/// Makes `rate` the default of the context
fn set_default_rate(conn: &Connection, context: i64, rate: &str) -> Result<()> {
    set_setting(conn, context, "tax_rate", rate.into())
}

/// The label of the unit new positions of the context start with, as it is stored
fn default_unit(conn: &Connection, context: i64) -> Result<Option<String>> {
    let json: String = conn
        .query_row("SELECT json FROM contexts WHERE id = ?1", [context], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| anyhow!("unknown context"))?;
    let body: ContextBody = serde_json::from_str(&json)?;
    Ok(body.settings.unit)
}

/// The default unit is the one called `from`: it follows the label to `to`, none clears it (back
/// to the first unit of the list)
fn follow_default_unit(
    conn: &Connection,
    context: i64,
    from: &str,
    to: Option<&str>,
) -> Result<()> {
    if default_unit(conn, context)?.as_deref() == Some(from) {
        set_setting(conn, context, "unit", to.into())?;
    }
    Ok(())
}

/// Where a rate of the list is used: by drafts and saved positions, and as the default
fn usage_of_rate(conn: &Connection, rate: &Rate) -> Result<RateUsage> {
    let key = rate_key(rate);
    let default = default_rate(conn, rate.context_id)?.as_deref() == Some(key.0.as_str());
    Ok(held(conn, rate.context_id, rate_key_of)?.usage_of(&key, default))
}

/// What names the rate `from` takes `to` instead, in the caller's transaction: the drafts of the
/// context, the saved positions it sees, and its default rate when that is the value of `from`
/// (the default has no label, a new label leaves it as it is).
fn follow_rate(
    conn: &Connection,
    context: i64,
    from: &(String, String),
    to: &(String, String),
) -> Result<()> {
    rewrite_positions(conn, context, &|p| retarget_rate(p, from, to))?;
    if from.0 != to.0 && default_rate(conn, context)?.as_deref() == Some(from.0.as_str()) {
        set_default_rate(conn, context, &to.0)?;
    }
    Ok(())
}

fn new_uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// UTC, in a form that sorts like time does
pub(crate) fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn set_last_backup_at(conn: &Connection, at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES ('last_backup_at', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [at],
    )?;
    Ok(())
}

/// Notes the time of a backup in a snapshot of the database, so the data that is restored from it
/// knows it was backed up then
pub fn stamp_snapshot(path: &Path, at: &str) -> Result<()> {
    let conn = Connection::open(path)?;
    ensure_app_state(&conn)?;
    set_last_backup_at(&conn, at)
}

/// Inserts or updates a layout row, returns its id
fn put(conn: &Connection, layout: &CustomLayout) -> Result<i64> {
    let json = serde_json::to_string(&CustomLayout {
        id: None,
        context_id: None,
        ..layout.clone()
    })?;
    if let Some(id) = layout.id {
        let changed = conn.execute(
            "UPDATE layouts SET json = ?1, context_id = ?2 WHERE id = ?3",
            params![json, layout.context_id, id],
        )?;
        if changed == 0 {
            bail!("unknown layout");
        }
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO layouts (json, context_id) VALUES (?1, ?2)",
            params![json, layout.context_id],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

/// A row of `invoices`, with what is still JSON text: the draft and the files
type StoredRow = (StoredInvoice, String, Option<String>);

fn stored(row: &Row) -> rusqlite::Result<StoredRow> {
    let draft: String = row.get("draft")?;
    let files: Option<String> = row.get("files")?;
    let status: String = row.get("status")?;
    Ok((
        StoredInvoice {
            id: row.get("id")?,
            context_id: row.get("context_id")?,
            kind: Kind::from_column(&row.get::<_, String>("kind")?),
            number: row.get("number")?,
            status: if status == "exported" {
                Status::Exported
            } else {
                Status::Draft
            },
            // filled below, `serde_json` errors do not fit rusqlite's error type
            draft: InvoiceDraft::blank(""),
            total: row.get("total")?,
            exported_at: row.get("exported_at")?,
            paid_at: row.get("paid_at")?,
            updated_at: row.get("updated_at")?,
            archived_at: row.get("archived_at")?,
            files: None,
        },
        draft,
        files,
    ))
}

/// What a backup holds, as the user counts it
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DataCounts {
    pub contexts: usize,
    pub invoices: usize,
    pub offers: usize,
    pub layouts: usize,
}

/// Opens the database file: write-ahead log, migrations
fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&conn)?;
    Ok(conn)
}

/// A connection that holds the place of the real one while its files are being replaced. It has no
/// tables, so a command that reaches it fails instead of writing into a database nobody keeps.
fn placeholder() -> Result<Connection> {
    Connection::open_in_memory_with_flags(
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .or_else(|_| Connection::open_in_memory())
    .map_err(Into::into)
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self(Mutex::new(open_connection(path)?)))
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    #[cfg(test)]
    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    // backup

    /// A consistent copy of the database in a new file. `VACUUM INTO` reads one transaction, the
    /// write-ahead log is part of it, which copying the files would not guarantee.
    pub fn snapshot_to(&self, target: &Path) -> Result<()> {
        let target = target
            .to_str()
            .ok_or_else(|| anyhow!("the path is not valid text"))?;
        self.conn()
            .execute("VACUUM INTO ?1", [target])
            .context("cannot copy the database")?;
        Ok(())
    }

    /// When the last backup was made (UTC, like the other times), none if there never was one
    pub fn last_backup_at(&self) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT value FROM app_state WHERE key = 'last_backup_at'",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_last_backup_at(&self, at: &str) -> Result<()> {
        set_last_backup_at(&self.conn(), at)
    }

    /// How much there is of what a backup holds
    pub fn counts(&self) -> Result<DataCounts> {
        let conn = self.conn();
        let count = |sql: &str| -> Result<usize> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
        Ok(DataCounts {
            contexts: count("SELECT COUNT(*) FROM contexts")?,
            invoices: count("SELECT COUNT(*) FROM invoices WHERE kind = 'invoice'")?,
            offers: count("SELECT COUNT(*) FROM invoices WHERE kind = 'offer'")?,
            layouts: count("SELECT COUNT(*) FROM layouts")?,
        })
    }

    /// Every kept original (PDF and XML) of every created document, with the hash it must have
    pub fn original_files(&self) -> Result<Vec<StoredFile>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT files FROM invoices WHERE files IS NOT NULL")?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut all = Vec::new();
        for json in rows {
            let files: InvoiceFiles = serde_json::from_str(&json)?;
            all.push(files.pdf);
            all.extend(files.xml);
        }
        Ok(all)
    }

    /// Lets `swap` change the database files, then opens the database at `path` again, migrated.
    /// The connection is closed meanwhile and the lock held, so no command sees half of it; the
    /// others wait. `swap` leaves nothing behind when it fails. When the new database does not open
    /// (it would not have been taken in then, but who knows), `undo` puts the old files back and
    /// the old database is opened again.
    pub fn replace_files<T>(
        &self,
        path: &Path,
        swap: impl FnOnce() -> Result<T>,
        undo: impl FnOnce(T),
    ) -> Result<T> {
        let mut guard = self.conn();
        // the log is folded into the file, so the file alone is the whole database
        guard.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        let old = std::mem::replace(&mut *guard, placeholder()?);
        if let Err((old, error)) = old.close() {
            *guard = old;
            return Err(error.into());
        }
        let reopen = |guard: &mut Connection| {
            open_connection(path).map(|conn| *guard = conn).context(
                "Die Datenbank ließ sich nicht wieder öffnen. Bitte e-rng beenden und neu starten.",
            )
        };
        let swapped = match swap() {
            Ok(swapped) => swapped,
            Err(error) => {
                reopen(&mut guard)?;
                return Err(error);
            }
        };
        match open_connection(path) {
            Ok(conn) => {
                *guard = conn;
                Ok(swapped)
            }
            Err(error) => {
                undo(swapped);
                reopen(&mut guard)?;
                Err(error.context("the restored database does not open"))
            }
        }
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // a panic elsewhere leaves SQLite itself consistent (it rolls back), so keep serving
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // contexts

    fn read_context(row: &Row) -> rusqlite::Result<(i64, String, Option<String>)> {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }

    fn to_context((id, json, archived_at): (i64, String, Option<String>)) -> Result<Context> {
        let body: ContextBody = serde_json::from_str(&json)?;
        Ok(Context {
            id: Some(id),
            brand: body.brand,
            archived_at,
            settings: body.settings.normalized(),
            missing: Vec::new(),
            missing_for_xrechnung: Vec::new(),
            missing_for_offers: Vec::new(),
        }
        .checked())
    }

    /// Every context, archived ones included, in the order they were made
    pub fn contexts(&self) -> Result<Vec<Context>> {
        let conn = self.conn();
        let mut statement =
            conn.prepare("SELECT id, json, archived_at FROM contexts ORDER BY id")?;
        let rows = statement.query_map([], Self::read_context)?;
        rows.map(|row| Self::to_context(row?)).collect()
    }

    pub fn context(&self, id: i64) -> Result<Context> {
        let row = self
            .conn()
            .query_row(
                "SELECT id, json, archived_at FROM contexts WHERE id = ?1",
                [id],
                Self::read_context,
            )
            .optional()?
            .ok_or_else(|| anyhow!("unknown context"))?;
        Self::to_context(row)
    }

    /// The settings of a context, those an invoice made under it is made with
    pub fn settings(&self, context_id: i64) -> Result<Settings> {
        Ok(self.context(context_id)?.settings)
    }

    /// Creates a context (`id` is `None`) or updates one. A new one without a brand gets the default
    /// for the first, a random one that is not in use for the others. Archiving is its own step.
    ///
    /// A new context starts with `units` and `rates` as its lists (none or empty: the defaults; the
    /// rate it charges is added to the rates when they lack it), an existing one keeps its own.
    pub fn save_context(
        &self,
        context: &Context,
        units: Option<&[UnitDef]>,
        rates: Option<&[RateDef]>,
    ) -> Result<Context> {
        let id = if let Some(id) = context.id {
            let brand = if context.brand.is_empty() {
                BRANDS[0].to_owned()
            } else {
                context.brand.clone()
            };
            let json = serde_json::to_string(&ContextBody {
                brand,
                settings: context.settings.clone(),
            })?;
            let changed = self.conn().execute(
                "UPDATE contexts SET json = ?1 WHERE id = ?2",
                params![json, id],
            )?;
            if changed == 0 {
                bail!("unknown context");
            }
            id
        } else {
            let brand = if context.brand.is_empty() {
                self.fresh_brand()?
            } else {
                context.brand.clone()
            };
            let json = serde_json::to_string(&ContextBody {
                brand,
                settings: context.settings.clone(),
            })?;
            let start = match units {
                Some(given) if !given.is_empty() => given.to_vec(),
                _ => units::defaults(),
            };
            // what is handed in is checked like a unit typed in, before the context exists, and
            // stored as the check normalizes it
            let start = start
                .iter()
                .enumerate()
                .map(|(i, unit)| {
                    let before: Vec<Unit> = start[..i].iter().map(|u| placed(0, u)).collect();
                    units::check(unit, &before)
                })
                .collect::<Result<Vec<_>>>()?;
            let start_rates = match rates {
                Some(given) if !given.is_empty() => given.to_vec(),
                _ => rates::defaults(),
            };
            let start_rates = start_rates
                .iter()
                .enumerate()
                .map(|(i, rate)| {
                    let before: Vec<Rate> =
                        start_rates[..i].iter().map(|r| placed_rate(0, r)).collect();
                    rates::check(rate, &before)
                })
                .collect::<Result<Vec<_>>>()?;
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            tx.execute("INSERT INTO contexts (json) VALUES (?1)", [json])?;
            let id = tx.last_insert_rowid();
            append_units(&tx, id, &start)?;
            append_rates(&tx, id, &start_rates)?;
            // the default is one of the list, whatever list the context was handed
            let charged = charged_entry(&rates_in(&tx, id)?, context.settings.tax_rate.trim());
            append_rates(&tx, id, &[charged])?;
            tx.commit()?;
            id
        };
        self.context(id)
    }

    /// The default for the first context, a random one that no other context shows for the next
    fn fresh_brand(&self) -> Result<String> {
        let used: Vec<String> = self.contexts()?.into_iter().map(|c| c.brand).collect();
        if used.is_empty() {
            return Ok(BRANDS[0].into());
        }
        let free: Vec<&str> = BRANDS
            .iter()
            .copied()
            .filter(|b| !used.iter().any(|u| u == b))
            .collect();
        let pool: Vec<&str> = if free.is_empty() {
            BRANDS.to_vec()
        } else {
            free
        };
        let pick = usize::from(uuid::Uuid::new_v4().as_bytes()[0]) % pool.len();
        Ok(pool[pick].into())
    }

    /// Hides a context from the switch, nothing of it is lost
    pub fn archive_context(&self, id: i64) -> Result<Context> {
        let changed = self.conn().execute(
            "UPDATE contexts SET archived_at = ?1 WHERE id = ?2 AND archived_at IS NULL",
            params![now(), id],
        )?;
        if changed == 0 {
            bail!("unknown or already archived context");
        }
        self.context(id)
    }

    /// Whether the layout is the default of any context that is in use
    pub fn is_default_layout(&self, key: &str) -> Result<bool> {
        Ok(self.contexts()?.iter().any(|c| {
            c.archived_at.is_none()
                && (c.settings.layout == key || c.settings.offer.layout.as_deref() == Some(key))
        }))
    }

    /// What belongs to a context
    pub fn context_usage(&self, id: i64) -> Result<ContextUsage> {
        let conn = self.conn();
        let count = |sql: &str| -> Result<usize> {
            Ok(usize::try_from(
                conn.query_row(sql, [id], |r| r.get::<_, i64>(0))?,
            )?)
        };
        Ok(ContextUsage {
            drafts: count(
                "SELECT COUNT(*) FROM invoices
                 WHERE context_id = ?1 AND kind = 'invoice' AND status = 'draft'",
            )?,
            exported: count(
                "SELECT COUNT(*) FROM invoices
                 WHERE context_id = ?1 AND kind = 'invoice' AND status = 'exported'",
            )?,
            offers: count(
                "SELECT COUNT(*) FROM invoices WHERE context_id = ?1 AND kind = 'offer'",
            )?,
            customers: count("SELECT COUNT(*) FROM customers WHERE context_id = ?1")?,
            templates: count("SELECT COUNT(*) FROM templates WHERE context_id = ?1")?,
            layouts: count("SELECT COUNT(*) FROM layouts WHERE context_id = ?1")?,
        })
    }

    /// Removes a context that has no invoices or offers (one that has is archived instead) and settles what is
    /// scoped to it: shared, moved to another context, or removed with it.
    pub fn remove_context(&self, id: i64, what: Disposition) -> Result<()> {
        self.context(id)?;
        let usage = self.context_usage(id)?;
        if usage.drafts + usage.exported + usage.offers > 0 {
            bail!("a context with invoices or offers cannot be removed, archive it instead");
        }
        if let Disposition::Move { to } = what {
            let target = self.context(to)?;
            if to == id || target.archived_at.is_some() {
                bail!("the things cannot move to that context");
            }
        }
        if what == Disposition::Delete {
            let layouts = self.scoped_layout_ids(id)?;
            if layouts
                .iter()
                .any(|layout| self.layout_usages(*layout).map_or(true, |u| !u.is_empty()))
            {
                bail!("a layout of the context is still used, it cannot be removed");
            }
        }
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        for table in ["customers", "templates", "layouts"] {
            match what {
                Disposition::Share => tx.execute(
                    &format!("UPDATE {table} SET context_id = NULL WHERE context_id = ?1"),
                    [id],
                )?,
                Disposition::Move { to } => tx.execute(
                    &format!("UPDATE {table} SET context_id = ?2 WHERE context_id = ?1"),
                    params![id, to],
                )?,
                Disposition::Delete => {
                    tx.execute(&format!("DELETE FROM {table} WHERE context_id = ?1"), [id])?
                }
            };
        }
        tx.execute("DELETE FROM units WHERE context_id = ?1", [id])?;
        tx.execute("DELETE FROM rates WHERE context_id = ?1", [id])?;
        // ids are reused, a later context must not inherit the floors
        tx.execute("DELETE FROM number_floors WHERE context_id = ?1", [id])?;
        tx.execute("DELETE FROM contexts WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    fn scoped_layout_ids(&self, context: i64) -> Result<Vec<i64>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id FROM layouts WHERE context_id = ?1")?;
        let ids = statement
            .query_map([context], |r| r.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    // customers

    /// The customers of a context: the shared ones and those that belong to it
    pub fn customers(&self, context: i64) -> Result<Vec<Customer>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, json, context_id FROM customers WHERE context_id IS NULL OR context_id = ?1",
        )?;
        let rows = statement.query_map([context], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let mut customers = rows
            .map(|row| {
                let (id, json, context_id) = row?;
                let mut customer: Customer = serde_json::from_str(&json)?;
                customer.id = Some(id);
                customer.context_id = context_id;
                Ok(customer)
            })
            .collect::<Result<Vec<_>>>()?;
        customers.sort_by_key(|c| c.name.to_lowercase());
        Ok(customers)
    }

    pub fn save_customer(&self, customer: &Customer) -> Result<Customer> {
        let conn = self.conn();
        let json = serde_json::to_string(&Customer {
            id: None,
            context_id: None,
            ..customer.clone()
        })?;
        let id = if let Some(id) = customer.id {
            let changed = conn.execute(
                "UPDATE customers SET json = ?1, context_id = ?2 WHERE id = ?3",
                params![json, customer.context_id, id],
            )?;
            if changed == 0 {
                bail!("unknown customer");
            }
            id
        } else {
            conn.execute(
                "INSERT INTO customers (json, context_id) VALUES (?1, ?2)",
                params![json, customer.context_id],
            )?;
            conn.last_insert_rowid()
        };
        Ok(Customer {
            id: Some(id),
            ..customer.clone()
        })
    }

    pub fn delete_customer(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM customers WHERE id = ?1", [id])?;
        Ok(())
    }

    // position templates

    /// The templates of a context: the shared ones and those that belong to it
    pub fn templates(&self, context: i64) -> Result<Vec<PositionTemplate>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, json, context_id FROM templates WHERE context_id IS NULL OR context_id = ?1",
        )?;
        let rows = statement.query_map([context], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let mut templates = rows
            .map(|row| {
                let (id, json, context_id) = row?;
                let mut template: PositionTemplate = serde_json::from_str(&json)?;
                template.id = Some(id);
                template.context_id = context_id;
                template.position = template.position.normalized();
                Ok(template)
            })
            .collect::<Result<Vec<_>>>()?;
        templates.sort_by_key(|t| t.position.description.to_lowercase());
        Ok(templates)
    }

    /// Saves a template. A new one that is the same as a saved one of its scope is not saved twice,
    /// the saved one comes back instead.
    pub fn save_template(&self, template: &PositionTemplate) -> Result<PositionTemplate> {
        if template.id.is_none() {
            let same_scope = |t: &PositionTemplate| t.context_id == template.context_id;
            // a shared template is compared with all of them, a scoped one with what its context sees
            let visible = match template.context_id {
                Some(context) => self.templates(context)?,
                None => self.all_templates()?,
            };
            if let Some(same) = visible
                .into_iter()
                .filter(same_scope)
                .find(|t| t.position.same_as(&template.position))
            {
                return Ok(same);
            }
        }
        let conn = self.conn();
        let json = serde_json::to_string(&PositionTemplate {
            id: None,
            context_id: None,
            ..template.clone()
        })?;
        let id = if let Some(id) = template.id {
            let changed = conn.execute(
                "UPDATE templates SET json = ?1, context_id = ?2 WHERE id = ?3",
                params![json, template.context_id, id],
            )?;
            if changed == 0 {
                bail!("unknown template");
            }
            id
        } else {
            conn.execute(
                "INSERT INTO templates (json, context_id) VALUES (?1, ?2)",
                params![json, template.context_id],
            )?;
            conn.last_insert_rowid()
        };
        Ok(PositionTemplate {
            id: Some(id),
            ..template.clone()
        })
    }

    fn all_templates(&self) -> Result<Vec<PositionTemplate>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id, json, context_id FROM templates")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, json, context_id) = row?;
            let mut template: PositionTemplate = serde_json::from_str(&json)?;
            template.id = Some(id);
            template.context_id = context_id;
            template.position = template.position.normalized();
            Ok(template)
        })
        .collect()
    }

    pub fn delete_template(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM templates WHERE id = ?1", [id])?;
        Ok(())
    }

    // units

    /// The units of a context in the order the user keeps them
    pub fn units(&self, context: i64) -> Result<Vec<Unit>> {
        units_in(&self.conn(), context)
    }

    /// Adds a unit to the end of a context's list (`id` is `None`) or changes one. A new label or
    /// code reaches the drafts and saved positions that used the old one, in the same step, and
    /// nothing else: created invoices keep the label in their positions.
    pub fn save_unit(&self, unit: &Unit) -> Result<Unit> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let saved = if let Some(id) = unit.id {
            let before = unit_row(&tx, id)?;
            let others: Vec<Unit> = units_in(&tx, before.context_id)?
                .into_iter()
                .filter(|u| u.id != Some(id))
                .collect();
            let def = units::check(
                &UnitDef {
                    code: unit.code.clone(),
                    label: unit.label.clone(),
                },
                &others,
            )?;
            tx.execute(
                "UPDATE units SET code = ?1, label = ?2 WHERE id = ?3",
                params![def.code, def.label, id],
            )?;
            let (from, to) = (
                (before.code.clone(), before.label.clone()),
                (def.code.clone(), def.label.clone()),
            );
            if from != to {
                rewrite_positions(&tx, before.context_id, &|p| retarget(p, &from, &to))?;
            }
            follow_default_unit(&tx, before.context_id, &before.label, Some(&def.label))?;
            Unit {
                id: Some(id),
                context_id: before.context_id,
                code: def.code,
                label: def.label,
            }
        } else {
            let known: bool = tx.query_row(
                "SELECT COUNT(*) > 0 FROM contexts WHERE id = ?1",
                [unit.context_id],
                |r| r.get(0),
            )?;
            if !known {
                bail!("unknown context");
            }
            let def = units::check(
                &UnitDef {
                    code: unit.code.clone(),
                    label: unit.label.clone(),
                },
                &units_in(&tx, unit.context_id)?,
            )?;
            append_units(&tx, unit.context_id, std::slice::from_ref(&def))?;
            Unit {
                id: Some(tx.last_insert_rowid()),
                context_id: unit.context_id,
                code: def.code,
                label: def.label,
            }
        };
        tx.commit()?;
        Ok(saved)
    }

    /// Where a unit is used by what changing or deleting it reaches
    pub fn unit_usage(&self, id: i64) -> Result<UnitUsage> {
        let conn = self.conn();
        let unit = unit_row(&conn, id)?;
        Ok(held(&conn, unit.context_id, key_of)?.usage_of(&(unit.code, unit.label)))
    }

    /// How many drafts and saved positions use each unit of a context, by unit id
    pub fn unit_counts(&self, context: i64) -> Result<std::collections::BTreeMap<i64, usize>> {
        let conn = self.conn();
        let held = held(&conn, context, key_of)?;
        Ok(units_in(&conn, context)?
            .into_iter()
            .filter_map(|u| {
                let usage = held.usage_of(&(u.code, u.label));
                Some((u.id?, usage.templates.len() + usage.drafts.len()))
            })
            .collect())
    }

    /// Deletes a unit. One that drafts or saved positions still use needs a `replacement` of the
    /// same context, they take it over in the same step. The last unit of a context stays.
    pub fn delete_unit(&self, id: i64, replacement: Option<i64>) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let unit = unit_row(&tx, id)?;
        if units_in(&tx, unit.context_id)?.len() < 2 {
            bail!("Die letzte Einheit kann nicht gelöscht werden.");
        }
        let target = match replacement {
            Some(other) if other == id => {
                bail!("Die Einheit kann nicht durch sich selbst ersetzt werden.")
            }
            Some(other) => {
                let target = unit_row(&tx, other)?;
                if target.context_id != unit.context_id {
                    bail!("Die Ersatz-Einheit gehört zu einem anderen Kontext.");
                }
                Some(target)
            }
            None => None,
        };
        let key = (unit.code, unit.label);
        if !held(&tx, unit.context_id, key_of)?
            .usage_of(&key)
            .is_empty()
        {
            let Some(target) = target else {
                bail!("Die Einheit wird noch verwendet, bitte eine Ersatz-Einheit wählen.");
            };
            let to = (target.code, target.label);
            rewrite_positions(&tx, unit.context_id, &|p| retarget(p, &key, &to))?;
        }
        tx.execute("DELETE FROM units WHERE id = ?1", [id])?;
        // no replacement for a default: the first unit of the list is it then
        follow_default_unit(&tx, unit.context_id, &key.1, None)?;
        tx.commit()?;
        Ok(())
    }

    /// Makes the unit called `label` the one new positions of a context start with, none: the
    /// first unit of the list. The label must be one of the list.
    pub fn choose_default_unit(&self, context: i64, label: Option<&str>) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let listed = label
            .map(|label| {
                units_in(&tx, context)?
                    .into_iter()
                    .find(|u| u.label == label)
                    .map(|u| u.label)
                    .ok_or_else(|| anyhow!("Diese Einheit gibt es nicht."))
            })
            .transpose()?;
        set_setting(&tx, context, "unit", listed.into())?;
        tx.commit()?;
        Ok(())
    }

    /// Puts the units of a context in the order of `ids`, which must name each of them once
    pub fn reorder_units(&self, context: i64, ids: &[i64]) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let mut have: Vec<i64> = units_in(&tx, context)?
            .into_iter()
            .filter_map(|u| u.id)
            .collect();
        let mut asked = ids.to_vec();
        have.sort_unstable();
        asked.sort_unstable();
        if have != asked {
            bail!("the order must name every unit of the context once");
        }
        for (position, id) in (0_i64..).zip(ids) {
            tx.execute(
                "UPDATE units SET sort = ?1 WHERE id = ?2",
                params![position, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // VAT rates

    /// The rates of a context in the order the user keeps them
    pub fn rates(&self, context: i64) -> Result<Vec<Rate>> {
        rates_in(&self.conn(), context)
    }

    /// Adds a rate to the end of a context's list (`id` is `None`) or changes one. A new value
    /// reaches the drafts and saved positions that used the old one, and the default of the
    /// context when it was that one, in the same step, and nothing else: created invoices keep the
    /// rate in their positions.
    pub fn save_rate(&self, rate: &Rate) -> Result<Rate> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let saved = if let Some(id) = rate.id {
            let before = rate_row(&tx, id)?;
            let others: Vec<Rate> = rates_in(&tx, before.context_id)?
                .into_iter()
                .filter(|r| r.id != Some(id))
                .collect();
            let def = rates::check(
                &RateDef {
                    label: rate.label.clone(),
                    rate: rate.rate.clone(),
                },
                &others,
            )?;
            tx.execute(
                "UPDATE rates SET label = ?1, rate = ?2 WHERE id = ?3",
                params![def.label, def.rate, id],
            )?;
            let saved = Rate {
                id: Some(id),
                context_id: before.context_id,
                label: def.label,
                rate: def.rate,
            };
            if rate_key(&before) != rate_key(&saved) {
                follow_rate(
                    &tx,
                    before.context_id,
                    &rate_key(&before),
                    &rate_key(&saved),
                )?;
            }
            saved
        } else {
            let known: bool = tx.query_row(
                "SELECT COUNT(*) > 0 FROM contexts WHERE id = ?1",
                [rate.context_id],
                |r| r.get(0),
            )?;
            if !known {
                bail!("unknown context");
            }
            let def = rates::check(
                &RateDef {
                    label: rate.label.clone(),
                    rate: rate.rate.clone(),
                },
                &rates_in(&tx, rate.context_id)?,
            )?;
            append_rates(&tx, rate.context_id, std::slice::from_ref(&def))?;
            Rate {
                id: Some(tx.last_insert_rowid()),
                context_id: rate.context_id,
                label: def.label,
                rate: def.rate,
            }
        };
        tx.commit()?;
        Ok(saved)
    }

    /// Where a rate is used by what changing or deleting it reaches
    pub fn rate_usage(&self, id: i64) -> Result<RateUsage> {
        let conn = self.conn();
        let rate = rate_row(&conn, id)?;
        usage_of_rate(&conn, &rate)
    }

    /// How many drafts and saved positions use each rate of a context, by rate id
    pub fn rate_counts(&self, context: i64) -> Result<std::collections::BTreeMap<i64, usize>> {
        let conn = self.conn();
        let held = held(&conn, context, rate_key_of)?;
        Ok(rates_in(&conn, context)?
            .into_iter()
            .filter_map(|r| {
                let usage = held.usage_of(&rate_key(&r), false);
                Some((r.id?, usage.templates.len() + usage.drafts.len()))
            })
            .collect())
    }

    /// Deletes a rate. One that drafts, saved positions or the context's default still use needs a
    /// `replacement` of the same context, they take it over in the same step.
    pub fn delete_rate(&self, id: i64, replacement: Option<i64>) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let rate = rate_row(&tx, id)?;
        let target = match replacement {
            Some(other) if other == id => {
                bail!("Der Steuersatz kann nicht durch sich selbst ersetzt werden.")
            }
            Some(other) => {
                let target = rate_row(&tx, other)?;
                if target.context_id != rate.context_id {
                    bail!("Der Ersatz-Steuersatz gehört zu einem anderen Kontext.");
                }
                Some(target)
            }
            None => None,
        };
        if !usage_of_rate(&tx, &rate)?.is_empty() {
            let Some(target) = target else {
                bail!("Der Steuersatz wird noch verwendet, bitte einen Ersatz wählen.");
            };
            follow_rate(&tx, rate.context_id, &rate_key(&rate), &rate_key(&target))?;
        }
        tx.execute("DELETE FROM rates WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Makes the rate of the list that `rate` names the default of a context, blank: no VAT is
    /// charged. A rate the list lacks is refused.
    pub fn choose_default_rate(&self, context: i64, rate: &str) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let chosen = if rate.trim().is_empty() {
            String::new()
        } else {
            let wanted = rates::canonical(rate);
            rates_in(&tx, context)?
                .into_iter()
                .find_map(|r| rates::canonical(&r.rate).filter(|c| Some(c) == wanted.as_ref()))
                .ok_or_else(|| anyhow!("Diesen Steuersatz gibt es nicht."))?
        };
        set_default_rate(&tx, context, &chosen)?;
        tx.commit()?;
        Ok(())
    }

    /// Puts the rates of a context in the order of `ids`, which must name each of them once
    pub fn reorder_rates(&self, context: i64, ids: &[i64]) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let mut have: Vec<i64> = rates_in(&tx, context)?
            .into_iter()
            .filter_map(|r| r.id)
            .collect();
        let mut asked = ids.to_vec();
        have.sort_unstable();
        asked.sort_unstable();
        if have != asked {
            bail!("the order must name every rate of the context once");
        }
        for (position, id) in (0_i64..).zip(ids) {
            tx.execute(
                "UPDATE rates SET sort = ?1 WHERE id = ?2",
                params![position, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // own layouts

    /// The own layouts a context can choose from: the shared ones and those that belong to it
    pub fn custom_layouts(&self, context: i64) -> Result<Vec<CustomLayout>> {
        Ok(self
            .all_custom_layouts()?
            .into_iter()
            .filter(|l| l.context_id.is_none_or(|c| c == context))
            .collect())
    }

    /// Own layouts of every context, by name: what rendering looks a layout up in. One that has no
    /// `uid` yet (made before there were any) gets it here, once.
    pub fn all_custom_layouts(&self) -> Result<Vec<CustomLayout>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id, json, context_id FROM layouts")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let mut layouts = rows
            .map(|row| {
                let (id, json, context_id) = row?;
                let mut layout: CustomLayout = serde_json::from_str(&json)?;
                layout.id = Some(id);
                layout.context_id = context_id;
                Ok(layout)
            })
            .collect::<Result<Vec<_>>>()?;
        drop(statement);
        for layout in layouts.iter_mut().filter(|l| l.uid.is_empty()) {
            layout.uid = new_uid();
            layout.updated_at = now();
            put(&conn, layout)?;
        }
        // fonts stored before their style was read get it from their file, once
        for layout in &mut layouts {
            let mut found = false;
            for font in layout.fonts.iter_mut().filter(|f| f.style.is_empty()) {
                if let Some(style) = crate::layout_files::style_of(&font.path) {
                    font.style = style;
                    found = true;
                }
            }
            if found {
                put(&conn, layout)?;
            }
        }
        layouts.sort_by_key(|l| l.name.to_lowercase());
        Ok(layouts)
    }

    /// Stores an own layout the user edited: a new one gets its `uid`, the time of saving is noted.
    pub fn save_custom_layout(&self, layout: &CustomLayout) -> Result<CustomLayout> {
        let mut layout = layout.clone();
        if layout.uid.is_empty() {
            layout.uid = new_uid();
        }
        layout.updated_at = now();
        self.store_layout(layout)
    }

    /// Stores a layout that came from a file as it is: its `uid` and time stay, so the copies can be
    /// compared later. A file that has none (the built-in layout) gets them like a new layout.
    pub fn store_imported_layout(&self, layout: &CustomLayout) -> Result<CustomLayout> {
        let mut layout = layout.clone();
        if layout.uid.is_empty() {
            layout.uid = new_uid();
        }
        if layout.updated_at.is_empty() {
            layout.updated_at = now();
        }
        self.store_layout(layout)
    }

    fn store_layout(&self, layout: CustomLayout) -> Result<CustomLayout> {
        let id = put(&self.conn(), &layout)?;
        Ok(CustomLayout {
            id: Some(id),
            ..layout
        })
    }

    /// Every own layout something still points at: the layouts themselves and the copies frozen into
    /// exported invoices.
    pub fn layouts_in_use(&self) -> Result<Vec<CustomLayout>> {
        let mut layouts = self.all_custom_layouts()?;
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT draft FROM invoices")?;
        for json in statement.query_map([], |r| r.get::<_, String>(0))? {
            let draft: InvoiceDraft = serde_json::from_str(&json?)?;
            layouts.extend(draft.frozen_layout);
        }
        Ok(layouts)
    }

    /// The invoices and offers (drafts and created ones) made with the own layout: chosen for
    /// themselves, or frozen into them on creation.
    pub fn layout_usages(&self, id: i64) -> Result<Vec<LayoutUsage>> {
        let key = format!("custom-{id}");
        let conn = self.conn();
        let mut statement =
            conn.prepare("SELECT id, kind, number, draft FROM invoices ORDER BY updated_at DESC")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut usages = Vec::new();
        for row in rows {
            let (invoice_id, kind, number, json) = row?;
            let draft: InvoiceDraft = serde_json::from_str(&json)?;
            let frozen = draft.frozen_layout.as_ref().and_then(|l| l.id) == Some(id);
            if draft.layout.as_deref() == Some(key.as_str()) || frozen {
                usages.push(LayoutUsage {
                    invoice_id,
                    kind: Kind::from_column(&kind),
                    number,
                    customer: draft.customer.name,
                });
            }
        }
        Ok(usages)
    }

    pub fn delete_custom_layout(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM layouts WHERE id = ?1", [id])?;
        Ok(())
    }

    // invoices

    /// The documents of a kind of a context, or of all (`None`), newest first. Archived ones only
    /// when asked.
    pub fn invoices(
        &self,
        context: Option<i64>,
        kind: Kind,
        with_archived: bool,
    ) -> Result<Vec<ListEntry>> {
        let sellers: std::collections::HashMap<i64, String> = self
            .contexts()?
            .into_iter()
            .filter_map(|c| Some((c.id?, c.settings.seller.name)))
            .collect();
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, context_id, kind, number, status, draft, total, exported_at, paid_at,
                 updated_at, archived_at, files,
                 (SELECT COUNT(*) FROM document_links
                  WHERE offer_id = invoices.id OR invoice_id = invoices.id) AS linked
             FROM invoices WHERE (?2 OR archived_at IS NULL) AND (?1 IS NULL OR context_id = ?1)
                 AND kind = ?3
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![context, with_archived, kind.as_str()], |row| {
            Ok((stored(row)?, row.get::<_, u32>("linked")?))
        })?;
        rows.map(|row| {
            let ((invoice, draft, _), linked) = row?;
            let draft: InvoiceDraft = serde_json::from_str(&draft)?;
            // an exported invoice shows the seller it was made with
            let seller_name = draft
                .frozen
                .as_ref()
                .map(|f| f.seller.name.clone())
                .or_else(|| sellers.get(&invoice.context_id).cloned())
                .unwrap_or_default();
            let valid_until = draft.valid_until().filter(|_| invoice.kind == Kind::Offer);
            Ok(ListEntry {
                id: invoice.id,
                context_id: invoice.context_id,
                kind: invoice.kind,
                seller_name,
                number: invoice.number,
                status: invoice.status,
                customer_name: draft.customer.name,
                issue_date: draft.issue_date,
                valid_until,
                total: invoice.total,
                paid_at: invoice.paid_at,
                updated_at: invoice.updated_at,
                archived_at: invoice.archived_at,
                linked,
            })
        })
        .collect()
    }

    pub fn invoice(&self, id: i64) -> Result<StoredInvoice> {
        let conn = self.conn();
        let (mut invoice, draft, files) = conn
            .query_row(
                "SELECT id, context_id, kind, number, status, draft, total, exported_at, paid_at,
                     updated_at, archived_at, files
                 FROM invoices WHERE id = ?1",
                [id],
                stored,
            )
            .optional()?
            .ok_or_else(|| anyhow!("unknown invoice"))?;
        invoice.draft = serde_json::from_str(&draft)?;
        // older rows have no labels, they are read as what the codes were printed as
        invoice.draft.positions = std::mem::take(&mut invoice.draft.positions)
            .into_iter()
            .map(Position::normalized)
            .collect();
        invoice.files = files.map(|f| serde_json::from_str(&f)).transpose()?;
        Ok(invoice)
    }

    /// Marks an exported invoice as paid on a date, or as open again (`None`). The invoice itself stays
    /// locked, this is the one thing that happens to it afterwards. Offers are not paid.
    pub fn set_paid(&self, id: i64, paid_at: Option<&str>) -> Result<StoredInvoice> {
        if let Some(date) = paid_at {
            en16931::Date::parse(date)
                .with_context(|| format!("the date of payment {date:?} is not a date"))?;
        }
        let changed = self.conn().execute(
            "UPDATE invoices SET paid_at = ?1 WHERE id = ?2 AND status = 'exported'
                 AND kind = 'invoice'",
            params![paid_at, id],
        )?;
        if changed == 0 {
            bail!("only an exported invoice can be paid");
        }
        self.invoice(id)
    }

    /// Creates a draft of a kind under a context (`id` is `None`) or updates one, which stays under
    /// the context and kind it was made under. Exported invoices are refused.
    pub fn save_draft(
        &self,
        id: Option<i64>,
        context_id: i64,
        kind: Kind,
        draft: &InvoiceDraft,
        total: Option<&str>,
    ) -> Result<StoredInvoice> {
        let json = serde_json::to_string(draft)?;
        let id = {
            let conn = self.conn();
            if let Some(id) = id {
                let changed = conn.execute(
                    "UPDATE invoices SET draft = ?1, total = ?2, updated_at = datetime('now')
                     WHERE id = ?3 AND status = 'draft'",
                    params![json, total, id],
                )?;
                if changed == 0 {
                    bail!("an exported invoice cannot be changed, duplicate it instead");
                }
                id
            } else {
                conn.execute(
                    "INSERT INTO invoices (context_id, kind, status, draft, total, updated_at)
                     VALUES (?1, ?2, 'draft', ?3, ?4, datetime('now'))",
                    params![context_id, kind.as_str(), json, total],
                )?;
                conn.last_insert_rowid()
            }
        };
        self.invoice(id)
    }

    /// Drafts only: exported invoices stay, their numbers must not disappear. Its links go with it.
    pub fn delete_draft(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let changed = tx.execute(
            "DELETE FROM invoices WHERE id = ?1 AND status = 'draft'",
            [id],
        )?;
        if changed == 0 {
            bail!("only drafts can be deleted");
        }
        tx.execute(
            "DELETE FROM document_links WHERE offer_id = ?1 OR invoice_id = ?1",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// A new draft with the content of an existing invoice or offer, dated `today`, in the same
    /// context and of the same kind. An offer keeps its validity days.
    pub fn duplicate(&self, id: i64, today: &str) -> Result<StoredInvoice> {
        let source = self.invoice(id)?;
        let mut draft = source.draft;
        draft.issue_date = today.into();
        draft.delivery_date = today.into();
        // a period stays a period, of today until changed
        draft.delivery_end = draft.delivery_end.map(|_| today.into());
        draft.frozen = None;
        draft.frozen_layout = None;
        draft.frozen_offers = Vec::new();
        self.save_draft(None, source.context_id, source.kind, &draft, None)
    }

    /// A new invoice draft from a created offer, linked to it, in one step. It has the customer,
    /// positions, buyer reference and note of the offer; layout and texts are the invoice defaults
    /// of `settings` (an offer's intro does not belong on an invoice), dated `today`.
    pub fn invoice_from_offer(
        &self,
        offer_id: i64,
        today: &str,
        settings: &Settings,
    ) -> Result<StoredInvoice> {
        let offer = self.invoice(offer_id)?;
        if offer.kind != Kind::Offer {
            bail!("only an offer can become an invoice");
        }
        if offer.status != Status::Exported {
            bail!("only a created offer can become an invoice");
        }
        let mut draft = settings.blank_draft(Kind::Invoice, today);
        draft.customer = offer.draft.customer;
        draft.positions = offer.draft.positions;
        draft.buyer_reference = offer.draft.buyer_reference;
        draft.note = offer.draft.note;
        // the list shows what the draft comes to, as `invoice_save` keeps it
        let total = build_invoice(&draft, settings, "ENTWURF")
            .ok()
            .map(|invoice| totals(&invoice).gross);
        let id = {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO invoices (context_id, kind, status, draft, total, updated_at)
                 VALUES (?1, 'invoice', 'draft', ?2, ?3, datetime('now'))",
                params![offer.context_id, serde_json::to_string(&draft)?, total],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO document_links (offer_id, invoice_id, created_at)
                 VALUES (?1, ?2, ?3)",
                params![offer_id, id, now()],
            )?;
            tx.commit()?;
            id
        };
        self.invoice(id)
    }

    /// The documents linked to an offer or an invoice, newest first
    pub fn links(&self, id: i64) -> Result<Vec<Linked>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT i.id, i.kind, i.number, i.status, i.draft
             FROM document_links l
             JOIN invoices i ON i.id = CASE WHEN l.offer_id = ?1 THEN l.invoice_id ELSE l.offer_id END
             WHERE l.offer_id = ?1 OR l.invoice_id = ?1
             ORDER BY i.updated_at DESC, i.id DESC",
        )?;
        let rows = statement.query_map([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, kind, number, status, draft) = row?;
            let draft: InvoiceDraft = serde_json::from_str(&draft)?;
            Ok(Linked {
                id,
                kind: Kind::from_column(&kind),
                number,
                status: if status == "exported" {
                    Status::Exported
                } else {
                    Status::Draft
                },
                customer_name: draft.customer.name,
                issue_date: draft.issue_date,
            })
        })
        .collect()
    }

    /// The numbers of the created offers linked to an invoice, oldest first: what an invoice that
    /// names its offers prints. A draft offer has no number yet and is left out.
    pub fn offer_numbers(&self, invoice_id: i64) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT o.number FROM document_links l JOIN invoices o ON o.id = l.offer_id
             WHERE l.invoice_id = ?1 AND o.number IS NOT NULL ORDER BY o.id",
        )?;
        let numbers = statement
            .query_map([invoice_id], |r| r.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(numbers)
    }

    /// Links an offer to an invoice of the same context, nothing happens when they are linked.
    /// Drafts and created documents can be linked, the link is no part of the document.
    pub fn link(&self, offer_id: i64, invoice_id: i64) -> Result<()> {
        let (offer, invoice) = (self.invoice(offer_id)?, self.invoice(invoice_id)?);
        if offer.kind != Kind::Offer || invoice.kind != Kind::Invoice {
            bail!("a link joins an offer and an invoice");
        }
        if offer.context_id != invoice.context_id {
            bail!("an offer and an invoice are linked within one context");
        }
        self.conn().execute(
            "INSERT OR IGNORE INTO document_links (offer_id, invoice_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![offer_id, invoice_id, now()],
        )?;
        Ok(())
    }

    /// Removes a link, nothing happens when there is none
    pub fn unlink(&self, offer_id: i64, invoice_id: i64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM document_links WHERE offer_id = ?1 AND invoice_id = ?2",
            params![offer_id, invoice_id],
        )?;
        Ok(())
    }

    /// As if the invoice had been exported by a version that did not keep files
    #[cfg(test)]
    pub fn forget_files(&self, id: i64) {
        self.conn()
            .execute("UPDATE invoices SET files = NULL WHERE id = ?1", [id])
            .unwrap();
    }

    /// Keeps the originals that were made again for an invoice exported without them
    pub fn set_files(&self, id: i64, files: &InvoiceFiles) -> Result<StoredInvoice> {
        let changed = self.conn().execute(
            "UPDATE invoices SET files = ?1 WHERE id = ?2 AND status = 'exported'
                 AND kind = 'invoice' AND files IS NULL",
            params![serde_json::to_string(files)?, id],
        )?;
        if changed == 0 {
            bail!("the invoice is not exported or has its files already");
        }
        self.invoice(id)
    }

    /// Takes an exported invoice out of the list, it stays as it is (and counts for its numbers).
    /// Drafts are deleted instead.
    pub fn archive_invoice(&self, id: i64) -> Result<()> {
        let changed = self.conn().execute(
            "UPDATE invoices SET archived_at = ?1 WHERE id = ?2 AND status = 'exported'
                 AND archived_at IS NULL",
            params![now(), id],
        )?;
        if changed == 0 {
            bail!("only an exported invoice can be archived");
        }
        Ok(())
    }

    /// Puts an archived invoice back into the list, as it was.
    pub fn unarchive_invoice(&self, id: i64) -> Result<StoredInvoice> {
        let changed = self.conn().execute(
            "UPDATE invoices SET archived_at = NULL WHERE id = ?1 AND archived_at IS NOT NULL",
            [id],
        )?;
        if changed == 0 {
            bail!("the invoice is not archived");
        }
        self.invoice(id)
    }

    /// Every number of a kind given in a context with its counter and the day it was issued on. A
    /// number that kept no counter (written before they were kept) is read back with the current
    /// pattern.
    fn given(
        conn: &Connection,
        context_id: i64,
        kind: Kind,
        pattern: &Pattern,
        calendar: Calendar,
    ) -> Result<Vec<Given>> {
        let mut statement = conn.prepare(
            "SELECT number, counter, issued_on, CASE WHEN counter IS NULL THEN draft END
             FROM invoices WHERE number IS NOT NULL AND context_id = ?1 AND kind = ?2",
        )?;
        let rows = statement
            .query_map(params![context_id, kind.as_str()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<u32>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(number, counter, issued_on, draft)| {
                let issued_on = issued_on.or_else(|| {
                    let draft: serde_json::Value = serde_json::from_str(&draft?).ok()?;
                    Some(draft["issue_date"].as_str()?.to_owned())
                });
                let date = Date::parse(&issued_on?).ok()?;
                let counter = counter.or_else(|| pattern.counter_of(&number, date, calendar))?;
                Some(Given {
                    number,
                    counter,
                    date,
                })
            })
            .collect())
    }

    fn floor(conn: &Connection, context_id: i64, kind: Kind, period: &str) -> Result<Option<u32>> {
        Ok(conn
            .query_row(
                "SELECT next FROM number_floors WHERE context_id = ?1 AND kind = ?2 AND period = ?3",
                params![context_id, kind.as_str(), period],
                |r| r.get(0),
            )
            .optional()?)
    }

    fn sequence_in(
        conn: &Connection,
        context_id: i64,
        kind: Kind,
        settings: &Settings,
        date: Date,
    ) -> Result<Sequence> {
        let pattern = Pattern::parse(settings.pattern(kind))?;
        let calendar = settings.calendar();
        let reset = settings.reset(kind);
        let period = calendar.period(reset, date);
        let given = Self::given(conn, context_id, kind, &pattern, calendar)?;
        let highest = calendar.highest(reset, &period, &given);
        let floor = Self::floor(conn, context_id, kind, period.key())?;
        let counter = highest.map_or(1, |g| g.counter + 1).max(floor.unwrap_or(0));
        Ok(Sequence {
            next_number: pattern.number(date, calendar, counter),
            counter,
            highest: highest.map(|g| g.number.clone()),
            // a floor the numbers have passed decides nothing any more
            floor: floor.filter(|floor| *floor > highest.map_or(0, |g| g.counter)),
        })
    }

    /// The next free number of a context for a document of a kind and that day, by the pattern of
    /// the kind, the business year of the settings and the floor of the period (see `numbering`).
    pub fn next_number(
        &self,
        context_id: i64,
        kind: Kind,
        settings: &Settings,
        date: Date,
    ) -> Result<String> {
        Ok(self.sequence(context_id, kind, settings, date)?.next_number)
    }

    /// Where the numbers of a kind stand in the period of a day
    pub fn sequence(
        &self,
        context_id: i64,
        kind: Kind,
        settings: &Settings,
        date: Date,
    ) -> Result<Sequence> {
        Self::sequence_in(&self.conn(), context_id, kind, settings, date)
    }

    /// Makes the sequence of a day's period continue from `next`, for numbers of another program.
    /// Refused at or below the highest number given in the period, a number is never given twice.
    /// One floor per period: it replaces the one before, and is dropped when it equals what the
    /// numbers lead to anyway.
    pub fn set_counter(
        &self,
        context_id: i64,
        kind: Kind,
        settings: &Settings,
        date: Date,
        next: u32,
    ) -> Result<Sequence> {
        // one lock for the check and the write
        let conn = self.conn();
        let pattern = Pattern::parse(settings.pattern(kind))?;
        let calendar = settings.calendar();
        let reset = settings.reset(kind);
        let period = calendar.period(reset, date);
        let given = Self::given(&conn, context_id, kind, &pattern, calendar)?;
        let highest = calendar.highest(reset, &period, &given);
        let lowest = highest.map_or(1, |g| g.counter + 1);
        if next < lowest {
            return Err(match highest {
                Some(g) => anyhow!(
                    "{} ist bereits vergeben, der Zähler kann nur auf {lowest} oder höher gesetzt werden.",
                    g.number
                ),
                None => anyhow!("Der Zähler beginnt bei 1."),
            });
        }
        if next == lowest {
            conn.execute(
                "DELETE FROM number_floors WHERE context_id = ?1 AND kind = ?2 AND period = ?3",
                params![context_id, kind.as_str(), period.key()],
            )?;
        } else {
            conn.execute(
                "INSERT INTO number_floors (context_id, kind, period, next, set_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))
                 ON CONFLICT (context_id, kind, period)
                 DO UPDATE SET next = excluded.next, set_at = excluded.set_at",
                params![context_id, kind.as_str(), period.key(), next],
            )?;
        }
        Self::sequence_in(&conn, context_id, kind, settings, date)
    }

    /// Makes a draft final: it gets the number of the sequence with its counter, freezes the
    /// settings and can no longer change.
    /// Exporting an already final invoice again is fine and does not come through here.
    pub fn finalize(
        &self,
        id: i64,
        sequence: &Sequence,
        frozen: &Settings,
        frozen_layout: Option<&CustomLayout>,
        total: &str,
        files: &InvoiceFiles,
    ) -> Result<StoredInvoice> {
        let stored = self.invoice(id)?;
        let kind = stored.kind;
        let mut draft = stored.draft;
        draft.frozen = Some(frozen.clone());
        draft.frozen_layout = frozen_layout.cloned();
        // what the invoice names, as `offer_numbers` told the render
        if kind == Kind::Invoice && draft.offer_reference {
            draft.frozen_offers = self.offer_numbers(id)?;
        }
        let changed = self.conn().execute(
            "UPDATE invoices SET number = ?1, counter = ?2, issued_on = ?3, status = 'exported',
                 draft = ?4, total = ?5, files = ?6, exported_at = datetime('now'),
                 updated_at = datetime('now')
             WHERE id = ?7 AND status = 'draft'",
            params![
                sequence.next_number,
                sequence.counter,
                draft.issue_date,
                serde_json::to_string(&draft)?,
                total,
                serde_json::to_string(files)?,
                id
            ],
        );
        match changed {
            Ok(1) => self.invoice(id),
            Ok(_) => bail!("the invoice is already exported"),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                bail!(
                    "the {} number {} is already taken",
                    kind.as_str(),
                    sequence.next_number
                )
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::fixtures::{customer, draft, settings};
    use crate::model::{Position, StoredFile};
    use crate::numbering::Reset;

    /// Files as a created invoice has them, the store only keeps what it is told
    fn files() -> InvoiceFiles {
        let file = |path: &str| StoredFile {
            path: path.into(),
            sha256: "0".repeat(64),
        };
        InvoiceFiles {
            pdf: file("invoices/1/x.pdf"),
            xml: Some(file("invoices/1/x.xml")),
            regenerated: false,
        }
    }

    /// The place in a sequence a document is given its number at
    fn at(number: &str, counter: u32) -> Sequence {
        Sequence {
            next_number: number.into(),
            counter,
            highest: None,
            floor: None,
        }
    }

    /// Settings that number by a pattern
    fn numbered(pattern: &str) -> Settings {
        Settings {
            number_pattern: pattern.into(),
            ..settings()
        }
    }

    fn context(store: &Store, seller: &str) -> Context {
        let mut settings = settings();
        settings.seller.name = seller.into();
        store
            .save_context(&Context::new(settings, ""), None, None)
            .unwrap()
    }

    #[test]
    fn contexts_are_created_updated_and_checked() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.contexts().unwrap(), Vec::new());

        let first = context(&store, "Alpha GmbH");
        assert_eq!(first.brand, "blue");
        assert_eq!(first.missing, Vec::<String>::new());

        let mut changed = first.clone();
        changed.settings.payment_days = Some(30);
        changed.settings.seller.iban.clear();
        let saved = store.save_context(&changed, None, None).unwrap();

        assert_eq!(
            store.settings(first.id.unwrap()).unwrap().payment_days,
            Some(30)
        );
        assert_eq!(saved.missing, ["seller.iban"]);
        assert!(store.settings(999).is_err());
    }

    #[test]
    fn a_new_context_gets_a_brand_that_is_not_in_use() {
        let store = Store::in_memory().unwrap();
        let mut brands = vec![context(&store, "A").brand];
        for name in ["B", "C", "D"] {
            brands.push(context(&store, name).brand);
        }

        let mut unique = brands.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), brands.len(), "{brands:?}");
        assert!(brands.iter().all(|b| BRANDS.contains(&b.as_str())));
    }

    #[test]
    fn archived_contexts_stay_but_are_marked() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A");

        let archived = store.archive_context(a.id.unwrap()).unwrap();

        assert!(archived.archived_at.is_some());
        assert_eq!(store.contexts().unwrap().len(), 1);
        assert!(store.archive_context(a.id.unwrap()).is_err());
    }

    #[test]
    fn customers_are_created_updated_sorted_and_deleted() {
        let store = Store::in_memory().unwrap();
        let b = store
            .save_customer(&Customer {
                name: "Bravo".into(),
                ..customer()
            })
            .unwrap();
        let a = store
            .save_customer(&Customer {
                name: "alpha".into(),
                ..customer()
            })
            .unwrap();

        assert_eq!(
            store
                .customers(1)
                .unwrap()
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "Bravo"]
        );

        store
            .save_customer(&Customer {
                name: "Charlie".into(),
                ..b.clone()
            })
            .unwrap();
        assert_eq!(store.customers(1).unwrap().last().unwrap().name, "Charlie");

        store.delete_customer(a.id.unwrap()).unwrap();
        assert_eq!(store.customers(1).unwrap().len(), 1);
        assert!(store
            .save_customer(&Customer {
                id: Some(999),
                ..customer()
            })
            .is_err());
    }

    #[test]
    fn the_same_template_is_not_saved_twice() {
        let store = Store::in_memory().unwrap();
        let template = |description: &str, price: &str| PositionTemplate {
            id: None,
            context_id: None,
            position: Position {
                description: description.into(),
                quantity: None,
                unit: None,
                unit_label: None,
                tax_rate: None,
                tax_label: None,
                price: price.into(),
            },
        };
        let first = store.save_template(&template("Beratung", "95")).unwrap();

        // another way to write the same thing
        let again = store
            .save_template(&template(" beratung ", "95,00"))
            .unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(store.templates(1).unwrap().len(), 1);

        // a different price is another template
        store.save_template(&template("Beratung", "120")).unwrap();
        assert_eq!(store.templates(1).unwrap().len(), 2);
    }

    #[test]
    fn templates_are_created_updated_sorted_and_deleted() {
        let store = Store::in_memory().unwrap();
        let template = |description: &str| PositionTemplate {
            id: None,
            context_id: None,
            position: Position {
                description: description.into(),
                quantity: Some("2".into()),
                unit: Some("HUR".into()),
                unit_label: None,
                tax_rate: None,
                tax_label: None,
                price: "95,00".into(),
            },
        };
        let b = store.save_template(&template("Beratung")).unwrap();
        let a = store.save_template(&template("analyse")).unwrap();

        let names = |store: &Store| {
            store
                .templates(1)
                .unwrap()
                .into_iter()
                .map(|t| t.position.description)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&store), ["analyse", "Beratung"]);
        // the position travels flat in the JSON, next to the id
        let json = serde_json::to_value(&b).unwrap();
        assert_eq!(json["description"], "Beratung");
        assert_eq!(json["id"], b.id.unwrap());

        store
            .save_template(&PositionTemplate {
                position: Position {
                    description: "Zeitaufwand".into(),
                    ..b.position.clone()
                },
                ..b.clone()
            })
            .unwrap();
        assert_eq!(names(&store), ["analyse", "Zeitaufwand"]);

        store.delete_template(a.id.unwrap()).unwrap();
        assert_eq!(names(&store), ["Zeitaufwand"]);
        assert!(store
            .save_template(&PositionTemplate {
                id: Some(999),
                ..template("x")
            })
            .is_err());
    }

    #[test]
    fn the_stand_in_connection_takes_no_writes() {
        let conn = placeholder().unwrap();

        assert!(conn.execute_batch("CREATE TABLE x (y);").is_err());
        assert!(conn
            .query_row("SELECT COUNT(*) FROM contexts", [], |r| r.get::<_, i64>(0))
            .is_err());
    }

    #[test]
    fn a_version_one_database_gets_the_templates_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL);
             CREATE TABLE customers (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             CREATE TABLE invoices (id INTEGER PRIMARY KEY, number TEXT UNIQUE, status TEXT NOT NULL,
                 draft TEXT NOT NULL, total TEXT, exported_at TEXT, updated_at TEXT NOT NULL);
             PRAGMA user_version = 1;",
        )
        .unwrap();

        let store = Store::init(conn).unwrap();

        assert_eq!(store.templates(1).unwrap(), Vec::new());
    }

    fn own_layout(name: &str) -> CustomLayout {
        CustomLayout {
            id: None,
            name: name.into(),
            fonts: Vec::new(),
            ..crate::layout::builtin()
        }
    }

    #[test]
    fn a_layout_keeps_its_uid_and_notes_when_it_was_saved() {
        let store = Store::in_memory().unwrap();
        let first = store.save_custom_layout(&own_layout("A")).unwrap();
        assert!(!first.uid.is_empty() && first.updated_at.ends_with('Z'));

        // renaming keeps what tells it from other layouts
        let renamed = store
            .save_custom_layout(&CustomLayout {
                name: "B".into(),
                ..first.clone()
            })
            .unwrap();
        assert_eq!(renamed.uid, first.uid);

        // an imported one keeps both, one without them gets them
        let imported = CustomLayout {
            uid: "u-1".into(),
            updated_at: "2020-01-01T00:00:00Z".into(),
            ..own_layout("C")
        };
        let stored = store.store_imported_layout(&imported).unwrap();
        assert_eq!(
            (stored.uid.as_str(), stored.updated_at.as_str()),
            ("u-1", "2020-01-01T00:00:00Z")
        );
        let plain = store.store_imported_layout(&own_layout("D")).unwrap();
        assert!(!plain.uid.is_empty() && !plain.updated_at.is_empty());

        // what is read back is what was stored, an old row without a uid gets one for good
        let conn = store.conn();
        conn.execute(
            "INSERT INTO layouts (json) VALUES (?1)",
            [serde_json::to_string(&own_layout("Old"))
                .unwrap()
                .replace("\"uid\":\"\",", "")],
        )
        .unwrap();
        drop(conn);
        let uid = |store: &Store| {
            store
                .custom_layouts(1)
                .unwrap()
                .into_iter()
                .find(|l| l.name == "Old")
                .unwrap()
                .uid
        };
        let once = uid(&store);
        assert!(!once.is_empty());
        assert_eq!(uid(&store), once);
    }

    #[test]
    fn own_layouts_are_saved_sorted_and_deleted() {
        let store = Store::in_memory().unwrap();
        let b = store.save_custom_layout(&own_layout("Beta")).unwrap();
        store.save_custom_layout(&own_layout("alpha")).unwrap();

        let names: Vec<_> = store
            .custom_layouts(1)
            .unwrap()
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(names, ["alpha", "Beta"]);

        let archived = CustomLayout {
            archived: true,
            ..b.clone()
        };
        store.save_custom_layout(&archived).unwrap();
        assert!(store.custom_layouts(1).unwrap().iter().any(|l| l.archived));

        store.delete_custom_layout(b.id.unwrap()).unwrap();
        assert_eq!(store.custom_layouts(1).unwrap().len(), 1);
    }

    #[test]
    fn a_layout_is_in_use_when_an_invoice_chose_it_or_was_exported_with_it() {
        let store = Store::in_memory().unwrap();
        let layout = store
            .save_custom_layout(&own_layout("Briefpapier"))
            .unwrap();
        let id = layout.id.unwrap();
        assert_eq!(store.layout_usages(id).unwrap().len(), 0);

        let mut chosen = draft();
        chosen.layout = Some(layout.key());
        store
            .save_draft(None, 1, Kind::Invoice, &chosen, None)
            .unwrap();
        assert_eq!(store.layout_usages(id).unwrap().len(), 1);

        let exported = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        store
            .finalize(
                exported,
                &at("RE-2026-001", 1),
                &settings(),
                Some(&layout),
                "1.00",
                &files(),
            )
            .unwrap();
        assert_eq!(store.layout_usages(id).unwrap().len(), 2);
    }

    #[test]
    fn an_exported_invoice_can_be_marked_paid_and_open_again() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        assert!(
            store.set_paid(id, Some("2026-10-20")).is_err(),
            "drafts are not paid"
        );

        store
            .finalize(
                id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        assert!(
            store.set_paid(id, Some("20.10.2026")).is_err(),
            "not an ISO date"
        );

        let paid = store.set_paid(id, Some("2026-10-20")).unwrap();
        assert_eq!(paid.paid_at.as_deref(), Some("2026-10-20"));
        assert_eq!(
            store.invoices(None, Kind::Invoice, false).unwrap()[0].paid_at,
            paid.paid_at
        );

        assert_eq!(store.set_paid(id, None).unwrap().paid_at, None);
    }

    #[test]
    fn a_database_stamped_back_to_the_first_version_migrates_again() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();

        migrate(&conn).unwrap();

        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn drafts_are_saved_listed_and_updated() {
        let store = Store::in_memory().unwrap();
        let first = store
            .save_draft(None, 1, Kind::Invoice, &draft(), Some("1181.08"))
            .unwrap();
        assert_eq!(first.status, Status::Draft);
        assert_eq!(first.number, None);

        let mut changed = draft();
        changed.note = Some("Danke".into());
        let updated = store
            .save_draft(Some(first.id), 1, Kind::Invoice, &changed, Some("2.00"))
            .unwrap();

        assert_eq!(updated.draft.note.as_deref(), Some("Danke"));
        let list = store.invoices(None, Kind::Invoice, false).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            (list[0].customer_name.as_str(), list[0].total.as_deref()),
            ("Beispiel AG", Some("2.00"))
        );
    }

    #[test]
    fn numbers_count_up_by_the_pattern() {
        let day = |y, m, d| Date::new(y, m, d).unwrap();
        let store = Store::in_memory().unwrap();
        let pattern = &settings();
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, pattern, day(2026, 3, 7))
                .unwrap(),
            "RE-2026-001"
        );

        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        store
            .finalize(
                id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        store
            .finalize(
                id,
                &at("RE-2026-007", 7),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();

        assert_eq!(
            store
                .next_number(1, Kind::Invoice, pattern, day(2026, 3, 7))
                .unwrap(),
            "RE-2026-008"
        );
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, pattern, day(2027, 1, 1))
                .unwrap(),
            "RE-2027-001"
        );
        // the pattern only formats, the counter goes on
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &numbered("[R-]YYYY-NNN"), day(2026, 1, 1))
                .unwrap(),
            "R-2026-008"
        );
        assert!(store
            .next_number(1, Kind::Invoice, &numbered("YYYY"), day(2026, 1, 1))
            .is_err());
    }

    #[test]
    fn exported_invoices_are_locked_and_keep_their_settings() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;

        let done = store
            .finalize(
                id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1181.08",
                &files(),
            )
            .unwrap();

        assert_eq!(done.status, Status::Exported);
        assert_eq!(done.number.as_deref(), Some("RE-2026-001"));
        assert_eq!(done.draft.frozen, Some(settings()));
        assert!(done.exported_at.is_some());
        assert!(store
            .save_draft(Some(id), 1, Kind::Invoice, &draft(), None)
            .is_err());
        assert!(store.delete_draft(id).is_err());
        assert!(store
            .finalize(
                id,
                &at("RE-2026-002", 2),
                &settings(),
                None,
                "1.00",
                &files()
            )
            .is_err());
    }

    #[test]
    fn a_number_is_never_given_twice() {
        let store = Store::in_memory().unwrap();
        let a = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        let b = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        store
            .finalize(
                a,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();

        let error = store
            .finalize(
                b,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("already taken"));
        assert_eq!(store.invoice(b).unwrap().status, Status::Draft);
    }

    #[test]
    fn duplicates_are_fresh_drafts_with_todays_date() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        let done = store
            .finalize(
                id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1181.08",
                &files(),
            )
            .unwrap();

        let copy = store.duplicate(done.id, "2026-11-02").unwrap();

        assert_ne!(copy.id, done.id);
        assert_eq!((copy.status, copy.number.as_deref()), (Status::Draft, None));
        assert_eq!(
            (
                copy.draft.issue_date.as_str(),
                copy.draft.delivery_date.as_str()
            ),
            ("2026-11-02", "2026-11-02")
        );
        assert_eq!(copy.draft.frozen, None);
        assert_eq!(copy.draft.positions, done.draft.positions);
    }

    #[test]
    fn only_drafts_can_be_deleted() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;

        store.delete_draft(id).unwrap();

        assert_eq!(
            store.invoices(None, Kind::Invoice, false).unwrap(),
            Vec::new()
        );
        assert!(store.delete_draft(id).is_err());
    }

    #[test]
    fn a_version_four_database_becomes_the_first_context() {
        let conn = Connection::open_in_memory().unwrap();
        let mut old = settings();
        old.seller.name = "Alt GmbH".into();
        conn.execute_batch(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL);
             CREATE TABLE customers (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             CREATE TABLE templates (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             CREATE TABLE layouts (id INTEGER PRIMARY KEY, json TEXT NOT NULL);
             CREATE TABLE invoices (id INTEGER PRIMARY KEY, number TEXT UNIQUE, status TEXT NOT NULL,
                 draft TEXT NOT NULL, total TEXT, exported_at TEXT, updated_at TEXT NOT NULL,
                 paid_at TEXT);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)",
            [serde_json::to_string(&old).unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO customers (json) VALUES (?1)",
            [serde_json::to_string(&customer()).unwrap()],
        )
        .unwrap();
        let json = serde_json::to_string(&draft()).unwrap();
        conn.execute(
            "INSERT INTO invoices (number, status, draft, total, exported_at, updated_at, paid_at)
             VALUES ('RE-2026-001', 'exported', ?1, '1.00', '2026-10-01T00:00:00Z', '2026-10-01', '2026-10-05')",
            [&json],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO invoices (status, draft, updated_at) VALUES ('draft', ?1, '2026-10-02')",
            [&json],
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 4;").unwrap();

        let store = Store::init(conn).unwrap();

        let contexts = store.contexts().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].id, Some(1));
        assert_eq!(contexts[0].brand, "blue");
        assert_eq!(contexts[0].settings, old);
        let list = store.invoices(None, Kind::Invoice, false).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|i| i.context_id == 1));
        let exported = list.iter().find(|i| i.number.is_some()).unwrap();
        assert_eq!(exported.paid_at.as_deref(), Some("2026-10-05"));
        assert_eq!(exported.seller_name, "Alt GmbH");
        assert_eq!(store.customers(1).unwrap().len(), 1);
        assert!(store.customers(1).unwrap()[0].context_id.is_none());
        // numbers are unique per context now: the old counter goes on
        assert_eq!(
            store
                .next_number(
                    1,
                    Kind::Invoice,
                    &settings(),
                    Date::new(2026, 10, 9).unwrap()
                )
                .unwrap(),
            "RE-2026-002"
        );
        let version: i32 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn a_fresh_database_has_no_context() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.contexts().unwrap(), Vec::new());
    }

    #[test]
    fn invoice_numbers_and_counters_are_per_context() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A").id.unwrap();
        let b = context(&store, "B").id.unwrap();
        let pattern = &settings();
        let day = Date::new(2026, 3, 7).unwrap();

        let first = store
            .save_draft(None, a, Kind::Invoice, &draft(), None)
            .unwrap();
        store
            .finalize(
                first.id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        assert_eq!(
            store.next_number(a, Kind::Invoice, pattern, day).unwrap(),
            "RE-2026-002"
        );
        assert_eq!(
            store.next_number(b, Kind::Invoice, pattern, day).unwrap(),
            "RE-2026-001"
        );

        // the same number in another context is fine, inside one it is not
        let other = store
            .save_draft(None, b, Kind::Invoice, &draft(), None)
            .unwrap();
        store
            .finalize(
                other.id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        let again = store
            .save_draft(None, a, Kind::Invoice, &draft(), None)
            .unwrap();
        assert!(store
            .finalize(
                again.id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files()
            )
            .is_err());
    }

    #[test]
    fn the_list_can_show_one_context_or_all_and_archived_invoices_when_asked() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "Alpha").id.unwrap();
        let b = context(&store, "Beta").id.unwrap();
        let in_a = store
            .save_draft(None, a, Kind::Invoice, &draft(), None)
            .unwrap();
        store
            .save_draft(None, b, Kind::Invoice, &draft(), None)
            .unwrap();

        assert_eq!(store.invoices(None, Kind::Invoice, false).unwrap().len(), 2);
        let only_a = store.invoices(Some(a), Kind::Invoice, false).unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].seller_name, "Alpha");

        assert!(
            store.archive_invoice(in_a.id).is_err(),
            "drafts are deleted, not archived"
        );
        store
            .finalize(
                in_a.id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        store.archive_invoice(in_a.id).unwrap();
        assert_eq!(
            store.invoices(Some(a), Kind::Invoice, false).unwrap(),
            Vec::new()
        );
        let with_archived = store.invoices(Some(a), Kind::Invoice, true).unwrap();
        assert_eq!(with_archived.len(), 1);
        assert!(with_archived[0].archived_at.is_some());
        // it still counts for the numbers
        assert_eq!(
            store
                .next_number(
                    a,
                    Kind::Invoice,
                    &settings(),
                    Date::new(2026, 5, 1).unwrap()
                )
                .unwrap(),
            "RE-2026-002"
        );
        assert!(store.archive_invoice(in_a.id).is_err());

        let restored = store.unarchive_invoice(in_a.id).unwrap();
        assert_eq!(restored.archived_at, None);
        assert_eq!(restored.number.as_deref(), Some("RE-2026-001"));
        assert_eq!(
            store.invoices(Some(a), Kind::Invoice, false).unwrap().len(),
            1
        );
        assert!(store.unarchive_invoice(in_a.id).is_err());
    }

    #[test]
    fn a_draft_stays_in_its_context_and_so_do_its_duplicates() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A").id.unwrap();
        let b = context(&store, "B").id.unwrap();
        let draft_in_b = store
            .save_draft(None, b, Kind::Invoice, &draft(), None)
            .unwrap();

        // saving again with another context does not move it
        let updated = store
            .save_draft(Some(draft_in_b.id), a, Kind::Invoice, &draft(), None)
            .unwrap();
        assert_eq!(updated.context_id, b);

        let done = store
            .finalize(
                draft_in_b.id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        assert_eq!(
            store.duplicate(done.id, "2026-11-02").unwrap().context_id,
            b
        );
    }

    #[test]
    fn customers_templates_and_layouts_can_belong_to_one_context() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A").id.unwrap();
        let b = context(&store, "B").id.unwrap();
        let name =
            |customers: Vec<Customer>| customers.into_iter().map(|c| c.name).collect::<Vec<_>>();
        for (name, context_id) in [("Shared", None), ("Only A", Some(a)), ("Only B", Some(b))] {
            store
                .save_customer(&Customer {
                    name: name.into(),
                    context_id,
                    ..customer()
                })
                .unwrap();
        }

        assert_eq!(name(store.customers(a).unwrap()), ["Only A", "Shared"]);
        assert_eq!(name(store.customers(b).unwrap()), ["Only B", "Shared"]);
        // the scope survives a round trip and an update
        let only_a = store.customers(a).unwrap()[0].clone();
        assert_eq!(only_a.context_id, Some(a));
        store
            .save_customer(&Customer {
                context_id: None,
                ..only_a
            })
            .unwrap();
        assert_eq!(store.customers(b).unwrap().len(), 3);

        let template = |description: &str, context_id| PositionTemplate {
            id: None,
            context_id,
            position: Position {
                description: description.into(),
                quantity: None,
                unit: None,
                unit_label: None,
                tax_rate: None,
                tax_label: None,
                price: "10".into(),
            },
        };
        store.save_template(&template("Beratung", None)).unwrap();
        store.save_template(&template("Beratung", Some(a))).unwrap();
        assert_eq!(store.templates(a).unwrap().len(), 2);
        assert_eq!(store.templates(b).unwrap().len(), 1);

        store
            .save_custom_layout(&CustomLayout {
                context_id: Some(a),
                ..own_layout("Nur A")
            })
            .unwrap();
        store.save_custom_layout(&own_layout("Alle")).unwrap();
        assert_eq!(store.custom_layouts(a).unwrap().len(), 2);
        assert_eq!(store.custom_layouts(b).unwrap().len(), 1);
        assert_eq!(store.all_custom_layouts().unwrap().len(), 2);
    }

    fn scoped(store: &Store, context: i64) {
        store
            .save_customer(&Customer {
                context_id: Some(context),
                ..customer()
            })
            .unwrap();
        store
            .save_template(&PositionTemplate {
                id: None,
                context_id: Some(context),
                position: Position {
                    description: "x".into(),
                    quantity: None,
                    unit: None,
                    unit_label: None,
                    tax_rate: None,
                    tax_label: None,
                    price: "1".into(),
                },
            })
            .unwrap();
        store
            .save_custom_layout(&CustomLayout {
                context_id: Some(context),
                ..own_layout("L")
            })
            .unwrap();
    }

    #[test]
    fn removing_a_context_settles_what_belongs_to_it() {
        use crate::context::Disposition::{Delete, Move, Share};
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A").id.unwrap();
        let b = context(&store, "B").id.unwrap();

        scoped(&store, a);
        store.remove_context(a, Share).unwrap();
        assert_eq!(store.contexts().unwrap().len(), 1);
        assert!(store
            .customers(b)
            .unwrap()
            .iter()
            .all(|c| c.context_id.is_none()));
        assert_eq!(store.customers(b).unwrap().len(), 1);

        let c = context(&store, "C").id.unwrap();
        scoped(&store, c);
        assert!(store.remove_context(c, Move { to: c }).is_err());
        store.remove_context(c, Move { to: b }).unwrap();
        assert_eq!(store.context_usage(b).unwrap().customers, 1);
        assert_eq!(store.context_usage(b).unwrap().layouts, 1);

        scoped(&store, b);
        assert_eq!(store.context_usage(b).unwrap().customers, 2);
        let customers = store.customers(b).unwrap().len();
        let spare = context(&store, "D").id.unwrap();
        store.remove_context(spare, Delete).unwrap();
        assert_eq!(store.customers(b).unwrap().len(), customers);

        let d = context(&store, "E").id.unwrap();
        scoped(&store, d);
        store.remove_context(d, Delete).unwrap();
        // shared from A, moved from C, scoped to B: the layout of E is gone
        assert_eq!(store.all_custom_layouts().unwrap().len(), 3);
        assert!(store.context(d).is_err());
    }

    #[test]
    fn a_context_with_invoices_cannot_be_removed() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A").id.unwrap();
        let b = context(&store, "B").id.unwrap();
        store
            .save_draft(None, a, Kind::Invoice, &draft(), None)
            .unwrap();

        let error = store
            .remove_context(a, crate::context::Disposition::Share)
            .unwrap_err();

        assert!(error.to_string().contains("archive"));
        assert!(store.context(a).is_ok());
        assert!(store
            .remove_context(b, crate::context::Disposition::Share)
            .is_ok());
    }

    #[test]
    fn a_layout_is_a_default_while_a_context_in_use_has_it_as_default() {
        let store = Store::in_memory().unwrap();
        let mut with_layout = settings();
        with_layout.layout = "custom-7".into();
        let a = store
            .save_context(&Context::new(with_layout, ""), None, None)
            .unwrap();
        assert!(store.is_default_layout("custom-7").unwrap());
        assert!(!store.is_default_layout("custom-8").unwrap());

        store.archive_context(a.id.unwrap()).unwrap();
        assert!(!store.is_default_layout("custom-7").unwrap());
    }

    #[test]
    fn the_files_of_a_created_invoice_are_kept_with_it() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        assert_eq!(store.invoice(id).unwrap().files, None);

        let done = store
            .finalize(
                id,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();

        assert_eq!(done.files, Some(files()));
        assert_eq!(store.invoice(id).unwrap().files, Some(files()));
    }

    #[test]
    fn files_that_were_made_again_are_only_added_to_an_exported_invoice_without_any() {
        let store = Store::in_memory().unwrap();
        let draft_id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        assert!(
            store.set_files(draft_id, &files()).is_err(),
            "a draft has none"
        );

        // an invoice exported before the files were kept
        store
            .conn()
            .execute(
                "UPDATE invoices SET status = 'exported', number = 'RE-1' WHERE id = ?1",
                [draft_id],
            )
            .unwrap();
        let again = InvoiceFiles {
            regenerated: true,
            ..files()
        };
        let done = store.set_files(draft_id, &again).unwrap();

        assert_eq!(done.files, Some(again.clone()));
        assert!(
            store.set_files(draft_id, &files()).is_err(),
            "kept originals stay"
        );
    }

    #[test]
    fn fonts_stored_without_their_style_get_it_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let font = crate::layout_files::import_font(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/OpenSans.ttf"),
            dir.path(),
        )
        .unwrap()
        .remove(0);
        assert_eq!(font.style, "Regular");
        let store = Store::in_memory().unwrap();
        // as an older version stored it
        let old = crate::model::FontFile {
            style: String::new(),
            ..font
        };
        store
            .save_custom_layout(&CustomLayout {
                fonts: vec![old],
                ..own_layout("Alt")
            })
            .unwrap();

        let layouts = store.custom_layouts(1).unwrap();

        assert_eq!(layouts[0].fonts[0].style, "Regular");
        // and it stays: the second read finds nothing to do
        assert_eq!(
            store.custom_layouts(1).unwrap()[0].fonts[0].style,
            "Regular"
        );
    }

    /// Gives a number with its counter to a new invoice of a context, issued on a day
    fn issue(store: &Store, context: i64, number: &str, counter: u32, issued_on: &str) {
        let draft = InvoiceDraft {
            issue_date: issued_on.into(),
            ..draft()
        };
        let id = store
            .save_draft(None, context, Kind::Invoice, &draft, None)
            .unwrap()
            .id;
        store
            .finalize(
                id,
                &at(number, counter),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
    }

    fn day(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    fn floors(store: &Store) -> i64 {
        store
            .conn()
            .query_row("SELECT COUNT(*) FROM number_floors", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_removed_context_leaves_no_floor_for_the_one_that_reuses_its_id() {
        let store = Store::in_memory().unwrap();
        let settings = settings();
        let day = Date::new(2026, 3, 7).unwrap();
        let gone = context(&store, "A").id.unwrap();
        store
            .set_counter(gone, Kind::Invoice, &settings, day, 88)
            .unwrap();
        assert_eq!(floors(&store), 1);

        store
            .remove_context(gone, crate::context::Disposition::Share)
            .unwrap();
        let reused = context(&store, "B").id.unwrap();

        assert_eq!(reused, gone);
        assert_eq!(floors(&store), 0);
        assert_eq!(
            store
                .next_number(reused, Kind::Invoice, &settings, day)
                .unwrap(),
            "RE-2026-001"
        );
    }

    #[test]
    fn a_counter_can_continue_from_a_number_of_another_program() {
        let store = Store::in_memory().unwrap();
        let settings = settings();
        let day = Date::new(2026, 3, 7).unwrap();

        let set = store
            .set_counter(1, Kind::Invoice, &settings, day, 88)
            .unwrap();

        assert_eq!(set.next_number, "RE-2026-088");
        assert_eq!((set.counter, set.floor, set.highest), (88, Some(88), None));
        assert_eq!(
            store.next_number(1, Kind::Invoice, &settings, day).unwrap(),
            "RE-2026-088"
        );
        // the floor is for this year, the next one starts at 1
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &settings, Date::new(2027, 1, 1).unwrap())
                .unwrap(),
            "RE-2027-001"
        );
        // other contexts do not see it
        assert_eq!(
            store.next_number(2, Kind::Invoice, &settings, day).unwrap(),
            "RE-2026-001"
        );

        issue(&store, 1, "RE-2026-088", 88, "2026-03-07");
        let after = store.sequence(1, Kind::Invoice, &settings, day).unwrap();
        assert_eq!(after.next_number, "RE-2026-089");
        // the numbers have passed the floor, it decides nothing any more
        assert_eq!(after.floor, None);
        assert_eq!(after.highest.as_deref(), Some("RE-2026-088"));
    }

    #[test]
    fn a_counter_at_or_below_the_highest_number_is_refused_naming_it() {
        let store = Store::in_memory().unwrap();
        let settings = settings();
        let day = Date::new(2026, 3, 7).unwrap();
        issue(&store, 1, "RE-2026-003", 3, "2026-02-01");
        issue(&store, 1, "RE-2026-087", 87, "2026-03-01");
        // another year does not count
        issue(&store, 1, "RE-2025-120", 120, "2025-03-01");

        for next in [87, 50, 1, 0] {
            let refused = store
                .set_counter(1, Kind::Invoice, &settings, day, next)
                .unwrap_err()
                .to_string();
            assert_eq!(
                refused,
                "RE-2026-087 ist bereits vergeben, der Zähler kann nur auf 88 oder höher gesetzt werden.",
                "{next}"
            );
        }
        assert_eq!(floors(&store), 0);
        assert_eq!(
            store.next_number(1, Kind::Invoice, &settings, day).unwrap(),
            "RE-2026-088"
        );

        let empty = Store::in_memory().unwrap();
        assert_eq!(
            empty
                .set_counter(1, Kind::Invoice, &settings, day, 0)
                .unwrap_err()
                .to_string(),
            "Der Zähler beginnt bei 1."
        );
    }

    #[test]
    fn a_floor_is_replaced_lowered_and_removed_at_the_number_that_follows_anyway() {
        let store = Store::in_memory().unwrap();
        let settings = settings();
        let day = Date::new(2026, 3, 7).unwrap();
        issue(&store, 1, "RE-2026-010", 10, "2026-03-01");

        store
            .set_counter(1, Kind::Invoice, &settings, day, 100)
            .unwrap();
        // lowering is fine while nothing at or above was given
        let lowered = store
            .set_counter(1, Kind::Invoice, &settings, day, 40)
            .unwrap();
        assert_eq!(lowered.next_number, "RE-2026-040");
        assert_eq!(floors(&store), 1, "one floor per period, replaced");

        // 11 is what the numbers lead to anyway
        let removed = store
            .set_counter(1, Kind::Invoice, &settings, day, 11)
            .unwrap();
        assert_eq!(
            (removed.next_number.as_str(), removed.floor),
            ("RE-2026-011", None)
        );
        assert_eq!(floors(&store), 0);
    }

    #[test]
    fn a_floor_belongs_to_the_period_whatever_the_pattern_writes() {
        let store = Store::in_memory().unwrap();
        let day = Date::new(2026, 3, 7).unwrap();
        store
            .set_counter(1, Kind::Invoice, &numbered("[RE-]YYYY-NNN"), day, 88)
            .unwrap();

        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &numbered("[RE-]YYYY-NNNN"), day)
                .unwrap(),
            "RE-2026-0088"
        );
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &numbered("[RG-]YYYY-NNN"), day)
                .unwrap(),
            "RG-2026-088"
        );
    }

    #[test]
    fn the_counter_goes_on_after_the_pattern_changed() {
        let store = Store::in_memory().unwrap();
        issue(&store, 1, "RE-2026-005", 5, "2026-03-01");

        // other text, another width: the counter is the one that was given
        assert_eq!(
            store
                .next_number(
                    1,
                    Kind::Invoice,
                    &numbered("[RG-]YYYY-NNNN"),
                    day(2026, 6, 1)
                )
                .unwrap(),
            "RG-2026-0006"
        );
        // a pattern that writes the month does not restart a counter that runs by year
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &numbered("YYYY-MM-NNN"), day(2026, 6, 1))
                .unwrap(),
            "2026-06-006"
        );
        // another year starts at 1
        assert_eq!(
            store
                .next_number(
                    1,
                    Kind::Invoice,
                    &numbered("[RG-]YYYY-NNNN"),
                    day(2027, 1, 1)
                )
                .unwrap(),
            "RG-2027-0001"
        );
    }

    #[test]
    fn the_counter_restarts_by_the_reset_and_not_by_what_the_pattern_writes() {
        let store = Store::in_memory().unwrap();
        issue(&store, 1, "202603-007", 7, "2026-03-20");
        let by = |reset, pattern: &str| Settings {
            number_reset: reset,
            ..numbered(pattern)
        };
        let next = |settings: &Settings, date| {
            store.next_number(1, Kind::Invoice, settings, date).unwrap()
        };

        // by month
        let month = by(Reset::Month, "YYYYMM-NNN");
        assert_eq!(next(&month, day(2026, 3, 31)), "202603-008");
        assert_eq!(next(&month, day(2026, 4, 1)), "202604-001");
        // by year the month in the number does not matter
        let year = by(Reset::FiscalYear, "YYYYMM-NNN");
        assert_eq!(next(&year, day(2026, 4, 1)), "202604-008");
        assert_eq!(next(&year, day(2027, 1, 1)), "202701-001");
        // by quarter
        let quarter = by(Reset::Quarter, "YYYY-[Q]Q-NNN");
        assert_eq!(next(&quarter, day(2026, 3, 31)), "2026-Q1-008");
        assert_eq!(next(&quarter, day(2026, 4, 1)), "2026-Q2-001");
        // never
        let never = by(Reset::Never, "YYYYMM-NNN");
        assert_eq!(next(&never, day(2030, 1, 1)), "203001-008");
    }

    #[test]
    fn a_fiscal_quarter_runs_over_new_year() {
        let store = Store::in_memory().unwrap();
        let settings = Settings {
            number_reset: Reset::Quarter,
            fiscal_year_start: Some(day(2026, 4, 1)),
            ..numbered("[RE-]GGGG-[Q]Q-NN")
        };
        issue(&store, 1, "RE-2026-Q3-04", 4, "2026-11-02");

        let next = |date| {
            store
                .next_number(1, Kind::Invoice, &settings, date)
                .unwrap()
        };

        assert_eq!(next(day(2026, 12, 31)), "RE-2026-Q3-05");
        assert_eq!(next(day(2027, 1, 1)), "RE-2026-Q4-01");
    }

    #[test]
    fn a_floor_follows_the_reset_of_the_settings() {
        let store = Store::in_memory().unwrap();
        let monthly = Settings {
            number_reset: Reset::Month,
            ..numbered("YYYYMM-NNN")
        };
        let never = Settings {
            number_reset: Reset::Never,
            ..numbered("[R]NNNN")
        };

        store
            .set_counter(1, Kind::Invoice, &monthly, day(2026, 3, 7), 50)
            .unwrap();
        store
            .set_counter(2, Kind::Invoice, &never, day(2026, 3, 7), 50)
            .unwrap();

        let next = |context, settings: &Settings, date| {
            store
                .next_number(context, Kind::Invoice, settings, date)
                .unwrap()
        };
        assert_eq!(next(1, &monthly, day(2026, 3, 31)), "202603-050");
        assert_eq!(next(1, &monthly, day(2026, 4, 1)), "202604-001");
        // without a reset it holds for good
        assert_eq!(next(2, &never, day(2031, 8, 1)), "R0050");
        // a floor of one reset says nothing to another one
        let yearly = Settings {
            number_reset: Reset::FiscalYear,
            ..numbered("YYYYMM-NNN")
        };
        assert_eq!(next(1, &yearly, day(2026, 3, 31)), "202603-001");
    }

    #[test]
    fn numbers_without_a_counter_are_read_back_by_the_pattern_of_the_settings() {
        let store = Store::in_memory().unwrap();
        let json = serde_json::to_string(&draft()).unwrap();
        // as an older version wrote it: no counter, no day of its own
        for number in ["RE-2026-004", "RE-2026-009", "RE-2025-077", "elsewhere"] {
            store
                .conn()
                .execute(
                    "INSERT INTO invoices (context_id, number, status, draft, total, exported_at,
                         updated_at)
                     VALUES (1, ?1, 'exported', ?2, '1.00', '2026-10-17', '2026-10-17')",
                    params![number, json],
                )
                .unwrap();
        }

        let sequence = store
            .sequence(1, Kind::Invoice, &settings(), day(2026, 11, 1))
            .unwrap();

        assert_eq!(sequence.next_number, "RE-2026-010");
        assert_eq!(sequence.highest.as_deref(), Some("RE-2026-009"));
        // the day of such a number is the issue date of its draft: 2026-10-17
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &settings(), day(2025, 11, 1))
                .unwrap(),
            "RE-2025-001"
        );
    }

    #[test]
    fn the_period_of_a_floor_follows_the_fiscal_year() {
        let store = Store::in_memory().unwrap();
        let fiscal = Settings {
            fiscal_year_start: Some(Date::new(2026, 4, 1).unwrap()),
            ..numbered("[RE-]GGGG-NNN")
        };
        let day = |y, m, d| Date::new(y, m, d).unwrap();

        store
            .set_counter(1, Kind::Invoice, &fiscal, day(2026, 10, 29), 88)
            .unwrap();

        // the same fiscal year over new year
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &fiscal, day(2027, 2, 1))
                .unwrap(),
            "RE-2026-088"
        );
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &fiscal, day(2027, 4, 1))
                .unwrap(),
            "RE-2027-001"
        );
        // without the fiscal year the number is not in the period any more
        assert_eq!(
            store
                .next_number(
                    1,
                    Kind::Invoice,
                    &numbered("[RE-]GGGG-NNN"),
                    day(2027, 2, 1)
                )
                .unwrap(),
            "RE-2027-001"
        );
    }

    #[test]
    fn a_version_six_database_gets_floors_counters_and_keeps_what_its_patterns_wrote() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch("DROP TABLE number_floors; PRAGMA user_version = 6;")
            .unwrap();
        let body = |pattern: &str| {
            serde_json::json!({
                "brand": "blue",
                "settings": {
                    "seller": serde_json::to_value(settings().seller).unwrap(),
                    "number_pattern": pattern,
                    "payment_days": 21,
                },
            })
            .to_string()
        };
        for (id, pattern) in [(1, "RG-YYYY-NNN"), (2, "Q-NNN"), (3, "[RE-]YYYY-NNN")] {
            conn.execute(
                "INSERT INTO contexts (id, json) VALUES (?1, ?2)",
                params![id, body(pattern)],
            )
            .unwrap();
        }
        let json = serde_json::to_string(&draft()).unwrap();
        conn.execute(
            "INSERT INTO invoices (context_id, number, status, draft, total, exported_at, paid_at,
                 updated_at, archived_at)
             VALUES (1, 'RG-2026-001', 'exported', ?1, '1.00', '2026-10-01', '2026-10-05',
                 '2026-10-01', '2026-10-09')",
            [&json],
        )
        .unwrap();

        let store = Store::init(conn).unwrap();

        let patterns: Vec<_> = store
            .contexts()
            .unwrap()
            .into_iter()
            .map(|c| c.settings.number_pattern)
            .collect();
        assert_eq!(patterns, ["R[G]-YYYY-NNN", "[Q]-NNN", "[RE-]YYYY-NNN"]);
        // nothing else of the stored settings moved
        assert_eq!(store.settings(1).unwrap().payment_days, Some(21));
        // the number goes on as before, also with a fiscal year in the settings
        let day = Date::new(2026, 5, 7).unwrap();
        let old = store.settings(1).unwrap();
        assert_eq!(
            store.next_number(1, Kind::Invoice, &old, day).unwrap(),
            "RG-2026-002"
        );
        let fiscal = Settings {
            fiscal_year_start: Some(Date::new(2026, 4, 1).unwrap()),
            ..old
        };
        assert_eq!(
            store.next_number(1, Kind::Invoice, &fiscal, day).unwrap(),
            "RG-2026-002"
        );
        // the invoice is as it was
        let list = store.invoices(Some(1), Kind::Invoice, true).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].number.as_deref(), Some("RG-2026-001"));
        assert_eq!(list[0].paid_at.as_deref(), Some("2026-10-05"));
        assert!(list[0].archived_at.is_some());
        assert_eq!(floors(&store), 0);
        for column in ["counter", "issued_on"] {
            assert!(has_column(&store.conn(), "invoices", column).unwrap());
        }
        let version: i32 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        // stamped back, it migrates again and leaves the patterns as they are
        store
            .conn()
            .execute_batch("PRAGMA user_version = 6;")
            .unwrap();
        migrate(&store.conn()).unwrap();
        assert_eq!(store.settings(1).unwrap().number_pattern, "R[G]-YYYY-NNN");
    }

    /// Creates a document of a kind with a number, an offer with a PDF only
    fn create_as(store: &Store, context: i64, kind: Kind, number: &str) -> StoredInvoice {
        let id = store
            .save_draft(None, context, kind, &draft(), None)
            .unwrap()
            .id;
        let files = InvoiceFiles {
            xml: files().xml.filter(|_| kind == Kind::Invoice),
            ..files()
        };
        // the counter is the last run of digits
        let counter = number
            .rsplit(|c: char| !c.is_ascii_digit())
            .find_map(|digits| digits.parse().ok())
            .unwrap_or(1);
        store
            .finalize(id, &at(number, counter), &settings(), None, "1.00", &files)
            .unwrap()
    }

    #[test]
    fn a_version_seven_database_gets_kinds_and_keeps_its_invoices() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        // the table as version 7 had it
        conn.execute_batch(
            "DROP TABLE invoices;
             CREATE TABLE invoices (
                 id INTEGER PRIMARY KEY,
                 context_id INTEGER NOT NULL,
                 number TEXT,
                 status TEXT NOT NULL,
                 draft TEXT NOT NULL,
                 total TEXT,
                 exported_at TEXT,
                 paid_at TEXT,
                 updated_at TEXT NOT NULL,
                 archived_at TEXT,
                 files TEXT,
                 counter INTEGER,
                 issued_on TEXT,
                 UNIQUE (context_id, number)
             );
             PRAGMA user_version = 7;",
        )
        .unwrap();
        let json = serde_json::to_string(&draft()).unwrap();
        let kept = serde_json::to_string(&files()).unwrap();
        conn.execute(
            "INSERT INTO invoices (context_id, number, status, draft, total, exported_at, paid_at,
                 updated_at, archived_at, files, counter, issued_on)
             VALUES (1, 'RE-2026-001', 'exported', ?1, '1.00', '2026-10-01', '2026-10-05',
                 '2026-10-01', '2026-10-09', ?2, 5, '2026-10-01'),
                 (1, NULL, 'draft', ?1, NULL, NULL, NULL, '2026-10-10', NULL, NULL, NULL, NULL)",
            params![json, kept],
        )
        .unwrap();

        let store = Store::init(conn).unwrap();

        let version: i32 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let created = store.invoice(1).unwrap();
        assert_eq!(created.kind, Kind::Invoice);
        assert_eq!(created.number.as_deref(), Some("RE-2026-001"));
        assert_eq!(created.paid_at.as_deref(), Some("2026-10-05"));
        assert!(created.archived_at.is_some());
        assert_eq!(created.files, Some(files()));
        assert_eq!(store.invoice(2).unwrap().kind, Kind::Invoice);
        assert_eq!(store.invoices(None, Kind::Invoice, true).unwrap().len(), 2);
        assert_eq!(store.invoices(None, Kind::Offer, true).unwrap(), Vec::new());
        // the stored counter goes on, not the one the number reads as
        let day = Date::new(2026, 5, 7).unwrap();
        assert_eq!(
            store
                .next_number(1, Kind::Invoice, &settings(), day)
                .unwrap(),
            "RE-2026-006"
        );

        // stamped back, it migrates again and keeps everything
        store
            .conn()
            .execute_batch("PRAGMA user_version = 7;")
            .unwrap();
        migrate(&store.conn()).unwrap();
        assert_eq!(store.invoices(None, Kind::Invoice, true).unwrap().len(), 2);
    }

    #[test]
    fn the_same_number_is_allowed_once_per_kind() {
        let store = Store::in_memory().unwrap();

        create_as(&store, 1, Kind::Invoice, "2026-001");
        let offer = create_as(&store, 1, Kind::Offer, "2026-001");

        assert_eq!(offer.kind, Kind::Offer);
        let again = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap()
            .id;
        let error = store
            .finalize(
                again,
                &at("2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("offer number 2026-001"));
        // another context has its own numbers
        create_as(&store, 2, Kind::Offer, "2026-001");
    }

    #[test]
    fn offers_and_invoices_have_a_reset_each() {
        let store = Store::in_memory().unwrap();
        let mut settings = settings();
        settings.offer.number_pattern = "[AN-]YYYYMM-NNN".into();
        settings.offer.number_reset = Reset::Month;
        create_as(&store, 1, Kind::Offer, "AN-202610-003");
        create_as(&store, 1, Kind::Invoice, "RE-2026-009");
        let next = |kind, date| store.next_number(1, kind, &settings, date).unwrap();

        // offers start again with the month, invoices with the year
        assert_eq!(next(Kind::Offer, day(2026, 10, 30)), "AN-202610-004");
        assert_eq!(next(Kind::Offer, day(2026, 11, 2)), "AN-202611-001");
        assert_eq!(next(Kind::Invoice, day(2026, 11, 2)), "RE-2026-010");
    }

    #[test]
    fn numbers_and_counters_are_per_kind() {
        let store = Store::in_memory().unwrap();
        let settings = settings();
        let day = Date::new(2026, 3, 7).unwrap();
        create_as(&store, 1, Kind::Invoice, "RE-2026-007");

        // the offers start at their own pattern and count on their own
        let first = store.next_number(1, Kind::Offer, &settings, day).unwrap();
        assert_eq!(first, "AN-2026-001");
        create_as(&store, 1, Kind::Offer, &first);
        assert_eq!(
            store.next_number(1, Kind::Offer, &settings, day).unwrap(),
            "AN-2026-002"
        );
        assert_eq!(
            store.next_number(1, Kind::Invoice, &settings, day).unwrap(),
            "RE-2026-008"
        );

        // and continue on their own: a floor of one kind does not touch the other
        store
            .set_counter(1, Kind::Offer, &settings, day, 50)
            .unwrap();
        assert_eq!(
            store.next_number(1, Kind::Offer, &settings, day).unwrap(),
            "AN-2026-050"
        );
        assert_eq!(
            store.next_number(1, Kind::Invoice, &settings, day).unwrap(),
            "RE-2026-008"
        );
        let error = store
            .set_counter(1, Kind::Offer, &settings, day, 1)
            .unwrap_err();
        assert!(error.to_string().contains("AN-2026-001"));
        assert_eq!(
            store
                .sequence(1, Kind::Invoice, &settings, day)
                .unwrap()
                .floor,
            None
        );
    }

    #[test]
    fn an_offer_is_not_paid_and_gets_no_files_made_again() {
        let store = Store::in_memory().unwrap();
        let offer = create_as(&store, 1, Kind::Offer, "AN-2026-001");
        let invoice = create_as(&store, 1, Kind::Invoice, "RE-2026-001");

        assert!(store.set_paid(offer.id, Some("2026-11-01")).is_err());
        assert!(store.set_paid(invoice.id, Some("2026-11-01")).is_ok());
        store.forget_files(offer.id);
        assert!(store.set_files(offer.id, &files()).is_err());
        assert_eq!(store.invoice(offer.id).unwrap().paid_at, None);
        assert_eq!(store.invoice(offer.id).unwrap().files, None);
        // an offer can be archived and put back like an invoice
        store.forget_files(invoice.id);
        assert!(store.set_files(invoice.id, &files()).is_ok());
        store.archive_invoice(offer.id).unwrap();
        assert!(store.unarchive_invoice(offer.id).is_ok());
    }

    #[test]
    fn the_lists_hold_one_kind_and_a_draft_keeps_its_kind() {
        let store = Store::in_memory().unwrap();
        let mut valid = draft();
        valid.validity_days = Some(30);
        let offer = store
            .save_draft(None, 1, Kind::Offer, &valid, None)
            .unwrap();
        store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap();

        let offers = store.invoices(None, Kind::Offer, false).unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].kind, Kind::Offer);
        assert_eq!(offers[0].valid_until.as_deref(), Some("2026-11-16"));
        let invoices = store.invoices(None, Kind::Invoice, false).unwrap();
        assert_eq!(invoices.len(), 1);
        assert_eq!(invoices[0].valid_until, None);

        // saving again keeps what it is, whatever the caller says
        let saved = store
            .save_draft(Some(offer.id), 1, Kind::Invoice, &valid, None)
            .unwrap();
        assert_eq!(saved.kind, Kind::Offer);

        // a copy is the same kind, with the same days counted from its own date
        let copy = store.duplicate(offer.id, "2026-12-01").unwrap();
        assert_eq!(copy.kind, Kind::Offer);
        assert_eq!(copy.draft.validity_days, Some(30));
        assert_eq!(copy.draft.valid_until().as_deref(), Some("2026-12-31"));
        assert_eq!(copy.draft.issue_date, "2026-12-01");
    }

    #[test]
    fn an_own_layout_is_in_use_by_offers_too() {
        let store = Store::in_memory().unwrap();
        let layout = store
            .save_custom_layout(&own_layout("Briefpapier"))
            .unwrap();
        let mut chosen = draft();
        chosen.layout = Some(layout.key());
        store
            .save_draft(None, 1, Kind::Offer, &chosen, None)
            .unwrap();
        store
            .save_draft(None, 1, Kind::Invoice, &chosen, None)
            .unwrap();

        let usages = store.layout_usages(layout.id.unwrap()).unwrap();

        let mut kinds: Vec<_> = usages.iter().map(|u| u.kind.as_str()).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, ["invoice", "offer"]);
        // and the layout an offer takes as default counts as one
        let mut with_offer_layout = settings();
        with_offer_layout.offer.layout = Some(layout.key());
        store
            .save_context(&Context::new(with_offer_layout, ""), None, None)
            .unwrap();
        assert!(store.is_default_layout(&layout.key()).unwrap());
    }

    #[test]
    fn a_context_with_created_offers_cannot_be_removed() {
        let store = Store::in_memory().unwrap();
        let a = context(&store, "A").id.unwrap();
        create_as(&store, a, Kind::Offer, "AN-2026-001");

        let usage = store.context_usage(a).unwrap();
        assert_eq!((usage.offers, usage.drafts, usage.exported), (1, 0, 0));
        let error = store
            .remove_context(a, crate::context::Disposition::Share)
            .unwrap_err();

        assert!(error.to_string().contains("archive"));
        assert!(store.context(a).is_ok());
        // a draft offer counts as well
        let b = context(&store, "B").id.unwrap();
        store
            .save_draft(None, b, Kind::Offer, &draft(), None)
            .unwrap();
        assert_eq!(store.context_usage(b).unwrap().offers, 1);
        assert!(store
            .remove_context(b, crate::context::Disposition::Share)
            .is_err());
    }

    /// A draft of a kind with a customer's name, so linked documents can be told apart
    fn draft_of(store: &Store, context: i64, kind: Kind, customer: &str) -> StoredInvoice {
        let mut named = draft();
        named.customer.name = customer.into();
        store.save_draft(None, context, kind, &named, None).unwrap()
    }

    #[test]
    fn a_version_eight_database_gets_links_and_keeps_its_documents() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch("DROP TABLE document_links; PRAGMA user_version = 8;")
            .unwrap();
        let json = serde_json::to_string(&draft()).unwrap();
        let kept = serde_json::to_string(&files()).unwrap();
        conn.execute(
            "INSERT INTO invoices (context_id, kind, number, status, draft, total, exported_at,
                 updated_at, files)
             VALUES (1, 'offer', 'AN-2026-001', 'exported', ?1, '1.00', '2026-10-01', '2026-10-01', ?2),
                 (1, 'invoice', NULL, 'draft', ?1, NULL, NULL, '2026-10-10', NULL)",
            params![json, kept],
        )
        .unwrap();

        let store = Store::init(conn).unwrap();

        let version: i32 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(
            store.invoice(1).unwrap().number.as_deref(),
            Some("AN-2026-001")
        );
        assert_eq!(
            store.invoices(None, Kind::Offer, true).unwrap()[0].linked,
            0
        );
        store.link(1, 2).unwrap();
        assert_eq!(store.links(2).unwrap().len(), 1);

        // stamped back, it migrates again and keeps the links
        store
            .conn()
            .execute_batch("PRAGMA user_version = 8;")
            .unwrap();
        migrate(&store.conn()).unwrap();
        assert_eq!(store.links(1).unwrap().len(), 1);
    }

    #[test]
    fn offers_and_invoices_are_linked_many_to_many() {
        let store = Store::in_memory().unwrap();
        let (a, b) = (
            create_as(&store, 1, Kind::Offer, "AN-2026-001"),
            draft_of(&store, 1, Kind::Offer, "Zweite GmbH"),
        );
        let (x, y) = (
            draft_of(&store, 1, Kind::Invoice, "Dritte KG"),
            create_as(&store, 1, Kind::Invoice, "RE-2026-001"),
        );

        store.link(a.id, x.id).unwrap();
        store.link(a.id, y.id).unwrap();
        store.link(b.id, x.id).unwrap();

        let of_offer: Vec<_> = store.links(a.id).unwrap().iter().map(|l| l.id).collect();
        assert_eq!(of_offer.len(), 2);
        assert!(of_offer.contains(&x.id) && of_offer.contains(&y.id));
        let of_invoice = store.links(x.id).unwrap();
        assert_eq!(of_invoice.len(), 2);
        assert!(of_invoice.iter().all(|l| l.kind == Kind::Offer));
        let created = store
            .links(y.id)
            .unwrap()
            .into_iter()
            .find(|l| l.id == a.id)
            .unwrap();
        assert_eq!(created.number.as_deref(), Some("AN-2026-001"));
        assert_eq!(created.status, Status::Exported);
        assert_eq!(created.customer_name, "Beispiel AG");
        assert_eq!(created.issue_date, "2026-10-17");
        let draft_end = store
            .links(a.id)
            .unwrap()
            .into_iter()
            .find(|l| l.id == x.id)
            .unwrap();
        assert_eq!(draft_end.number, None);
        assert_eq!(draft_end.status, Status::Draft);
        assert_eq!(draft_end.customer_name, "Dritte KG");
        assert_eq!(store.links(b.id).unwrap().len(), 1);
    }

    #[test]
    fn an_invoice_that_names_its_offers_freezes_their_numbers_when_it_is_created() {
        let store = Store::in_memory().unwrap();
        let first = create_as(&store, 1, Kind::Offer, "AN-2026-001");
        let second = create_as(&store, 1, Kind::Offer, "AN-2026-002");
        let open = draft_of(&store, 1, Kind::Offer, "Dritte KG");
        let naming = InvoiceDraft {
            offer_reference: true,
            ..draft()
        };
        let named = store
            .save_draft(None, 1, Kind::Invoice, &naming, None)
            .unwrap()
            .id;
        let silent = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        for offer in [second.id, first.id, open.id] {
            store.link(offer, named).unwrap();
            store.link(offer, silent).unwrap();
        }

        // oldest first, an offer that has no number yet is left out
        assert_eq!(
            store.offer_numbers(named).unwrap(),
            ["AN-2026-001", "AN-2026-002"]
        );

        let created = store
            .finalize(
                named,
                &at("RE-2026-001", 1),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        let unnamed = store
            .finalize(
                silent,
                &at("RE-2026-002", 2),
                &settings(),
                None,
                "1.00",
                &files(),
            )
            .unwrap();
        assert_eq!(created.draft.frozen_offers, ["AN-2026-001", "AN-2026-002"]);
        assert!(unnamed.draft.frozen_offers.is_empty());

        // a link that changes later does not change what the invoice froze
        store.unlink(first.id, named).unwrap();
        assert_eq!(
            store.invoice(named).unwrap().draft.frozen_offers,
            ["AN-2026-001", "AN-2026-002"]
        );
        // a copy keeps the choice, and none of what was frozen
        let copy = store.duplicate(named, "2026-10-20").unwrap();
        assert!(copy.draft.offer_reference);
        assert!(copy.draft.frozen_offers.is_empty());
    }

    #[test]
    fn an_invoice_from_an_offer_starts_with_the_choice_of_the_settings() {
        let store = Store::in_memory().unwrap();
        let offer = create_as(&store, 1, Kind::Offer, "AN-2026-001");
        let mut settings = settings();

        let silent = store
            .invoice_from_offer(offer.id, "2026-10-20", &settings)
            .unwrap();
        settings.offer_reference = true;
        let naming = store
            .invoice_from_offer(offer.id, "2026-10-20", &settings)
            .unwrap();

        assert!(!silent.draft.offer_reference);
        assert!(naming.draft.offer_reference);
        assert_eq!(store.offer_numbers(naming.id).unwrap(), ["AN-2026-001"]);
    }

    #[test]
    fn a_link_joins_an_offer_and_an_invoice_of_one_context() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let second = context(&store, "B").id.unwrap();
        let offer = draft_of(&store, first, Kind::Offer, "X");
        let other_offer = draft_of(&store, first, Kind::Offer, "Y");
        let invoice = draft_of(&store, first, Kind::Invoice, "X");
        let other_invoice = draft_of(&store, first, Kind::Invoice, "Y");
        let elsewhere = draft_of(&store, second, Kind::Invoice, "X");

        let across = store.link(offer.id, elsewhere.id).unwrap_err();
        assert!(across.to_string().contains("one context"));
        assert!(store.link(offer.id, other_offer.id).is_err());
        assert!(store.link(invoice.id, other_invoice.id).is_err());
        // the offer goes first
        assert!(store.link(invoice.id, offer.id).is_err());
        assert!(store.link(offer.id, 9999).is_err());

        assert_eq!(store.links(offer.id).unwrap(), Vec::new());
        assert_eq!(store.links(elsewhere.id).unwrap(), Vec::new());
    }

    #[test]
    fn linking_twice_is_one_link_and_unlinking_removes_it() {
        let store = Store::in_memory().unwrap();
        let offer = draft_of(&store, 1, Kind::Offer, "X");
        let invoice = draft_of(&store, 1, Kind::Invoice, "X");

        store.link(offer.id, invoice.id).unwrap();
        store.link(offer.id, invoice.id).unwrap();
        assert_eq!(store.links(offer.id).unwrap().len(), 1);
        assert_eq!(store.links(invoice.id).unwrap().len(), 1);

        store.unlink(offer.id, invoice.id).unwrap();
        assert_eq!(store.links(offer.id).unwrap(), Vec::new());
        assert_eq!(store.links(invoice.id).unwrap(), Vec::new());
        // and again is fine
        store.unlink(offer.id, invoice.id).unwrap();
    }

    #[test]
    fn links_can_be_changed_on_created_documents() {
        let store = Store::in_memory().unwrap();
        let offer = create_as(&store, 1, Kind::Offer, "AN-2026-001");
        let invoice = create_as(&store, 1, Kind::Invoice, "RE-2026-001");

        store.link(offer.id, invoice.id).unwrap();
        assert_eq!(store.links(offer.id).unwrap().len(), 1);
        store.unlink(offer.id, invoice.id).unwrap();
        assert_eq!(store.links(offer.id).unwrap(), Vec::new());
    }

    #[test]
    fn a_deleted_draft_takes_its_links_and_an_archived_document_keeps_them() {
        let store = Store::in_memory().unwrap();
        let offer = create_as(&store, 1, Kind::Offer, "AN-2026-001");
        let invoice = draft_of(&store, 1, Kind::Invoice, "X");
        let kept = create_as(&store, 1, Kind::Invoice, "RE-2026-001");
        store.link(offer.id, invoice.id).unwrap();
        store.link(offer.id, kept.id).unwrap();

        store.archive_invoice(offer.id).unwrap();
        store.archive_invoice(kept.id).unwrap();
        assert_eq!(store.links(offer.id).unwrap().len(), 2);

        store.delete_draft(invoice.id).unwrap();
        let left = store.links(offer.id).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, kept.id);
        // a refused deletion keeps the links as well
        assert!(store.delete_draft(kept.id).is_err());
        assert_eq!(store.links(kept.id).unwrap().len(), 1);
    }

    #[test]
    fn the_list_counts_the_links_of_each_entry() {
        let store = Store::in_memory().unwrap();
        let (a, b) = (
            draft_of(&store, 1, Kind::Offer, "X"),
            draft_of(&store, 1, Kind::Offer, "Y"),
        );
        let (x, y) = (
            draft_of(&store, 1, Kind::Invoice, "X"),
            draft_of(&store, 1, Kind::Invoice, "Y"),
        );
        store.link(a.id, x.id).unwrap();
        store.link(a.id, y.id).unwrap();

        let linked = |kind: Kind| -> std::collections::HashMap<i64, u32> {
            store
                .invoices(None, kind, true)
                .unwrap()
                .into_iter()
                .map(|e| (e.id, e.linked))
                .collect()
        };
        let offers = linked(Kind::Offer);
        assert_eq!((offers[&a.id], offers[&b.id]), (2, 0));
        let invoices = linked(Kind::Invoice);
        assert_eq!((invoices[&x.id], invoices[&y.id]), (1, 1));
    }

    /// Settings in which offers and invoices differ in layout and texts
    fn apart() -> Settings {
        let mut apart = settings();
        apart.layout = "standard".into();
        apart.intro = Some("Wir berechnen Ihnen:".into());
        apart.closing = Some("Zahlbar sofort.".into());
        apart.offer.layout = Some("angebot".into());
        apart.offer.intro = Some("Gerne bieten wir an:".into());
        apart.offer.closing = Some("Gültig bis {valid_until}.".into());
        apart
    }

    #[test]
    fn an_invoice_draft_from_an_offer_has_its_content_and_the_invoice_defaults() {
        let store = Store::in_memory().unwrap();
        let mut written = draft();
        written.buyer_reference = Some("991-01234-56".into());
        written.note = Some("Aufwand nach Stunden".into());
        written.validity_days = Some(30);
        written.layout = Some("angebot".into());
        written.intro = Some("Gerne bieten wir an:".into());
        written.delivery_end = Some("2026-10-20".into());
        let id = store
            .save_draft(None, 1, Kind::Offer, &written, None)
            .unwrap()
            .id;
        let offer = store
            .finalize(id, &at("AN-2026-001", 1), &apart(), None, "1.00", &files())
            .unwrap();

        let invoice = store
            .invoice_from_offer(offer.id, "2026-12-01", &apart())
            .unwrap();

        assert_eq!(invoice.kind, Kind::Invoice);
        assert_eq!(invoice.status, Status::Draft);
        assert_eq!(invoice.number, None);
        assert_eq!(invoice.context_id, offer.context_id);
        let made = &invoice.draft;
        assert_eq!(made.customer, offer.draft.customer);
        assert_eq!(made.positions, offer.draft.positions);
        assert_eq!(made.buyer_reference.as_deref(), Some("991-01234-56"));
        assert_eq!(made.note.as_deref(), Some("Aufwand nach Stunden"));
        // dated today, nothing of the offer's validity or its frozen state
        assert_eq!(made.issue_date, "2026-12-01");
        assert_eq!(made.delivery_date, "2026-12-01");
        assert_eq!(made.delivery_end, None);
        assert_eq!(made.validity_days, None);
        assert!(made.frozen.is_none() && made.frozen_layout.is_none());
        // layout and texts are those of invoices
        assert_eq!(made.layout.as_deref(), Some("standard"));
        assert_eq!(made.intro.as_deref(), Some("Wir berechnen Ihnen:"));
        assert_eq!(made.closing.as_deref(), Some("Zahlbar sofort."));
        // the draft shows what it comes to
        assert_eq!(invoice.total.as_deref(), Some("1181.08"));
        // and it is linked to the offer
        let links = store.links(invoice.id).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, offer.id);
        assert_eq!(links[0].number.as_deref(), Some("AN-2026-001"));
        assert_eq!(
            store.invoices(None, Kind::Offer, true).unwrap()[0].linked,
            1
        );

        // again: one more draft, linked as well
        let second = store
            .invoice_from_offer(offer.id, "2026-12-01", &apart())
            .unwrap();
        assert_ne!(second.id, invoice.id);
        assert_eq!(store.links(offer.id).unwrap().len(), 2);
    }

    #[test]
    fn only_a_created_offer_becomes_an_invoice_draft() {
        let store = Store::in_memory().unwrap();
        let drafted = draft_of(&store, 1, Kind::Offer, "X");
        let invoice = create_as(&store, 1, Kind::Invoice, "RE-2026-001");

        for id in [drafted.id, invoice.id, 9999] {
            assert!(store
                .invoice_from_offer(id, "2026-12-01", &apart())
                .is_err());
        }

        // nothing was stored
        assert_eq!(store.invoices(None, Kind::Invoice, true).unwrap().len(), 1);
        assert_eq!(store.links(drafted.id).unwrap(), Vec::new());
    }

    // units

    fn position(description: &str, unit: Option<&str>, label: Option<&str>) -> Position {
        Position {
            description: description.into(),
            quantity: None,
            unit: unit.map(Into::into),
            unit_label: label.map(Into::into),
            tax_rate: None,
            tax_label: None,
            price: "1".into(),
        }
    }

    fn saved_position(
        store: &Store,
        context: Option<i64>,
        unit: Option<&str>,
        label: Option<&str>,
    ) -> PositionTemplate {
        store
            .save_template(&PositionTemplate {
                id: None,
                context_id: context,
                position: position("Leistung", unit, label),
            })
            .unwrap()
    }

    /// A draft of a context with one position that names a unit
    fn draft_with_unit(store: &Store, context: i64, unit: &str, label: &str) -> StoredInvoice {
        let mut named = draft();
        named.positions = vec![position("Leistung", Some(unit), Some(label))];
        store
            .save_draft(None, context, Kind::Invoice, &named, None)
            .unwrap()
    }

    fn unit_of(store: &Store, context: i64, label: &str) -> Unit {
        store
            .units(context)
            .unwrap()
            .into_iter()
            .find(|u| u.label == label)
            .unwrap()
    }

    fn labels(store: &Store, context: i64) -> Vec<String> {
        store
            .units(context)
            .unwrap()
            .into_iter()
            .map(|u| u.label)
            .collect()
    }

    fn new_unit(context: i64, code: &str, label: &str) -> Unit {
        Unit {
            id: None,
            context_id: context,
            code: code.into(),
            label: label.into(),
        }
    }

    /// What is stored of a draft or an exported invoice, byte for byte
    fn stored_draft(store: &Store, id: i64) -> String {
        store
            .conn()
            .query_row("SELECT draft FROM invoices WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    }

    fn stored_template(store: &Store, id: i64) -> String {
        store
            .conn()
            .query_row("SELECT json FROM templates WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    }

    #[test]
    fn a_version_nine_database_gets_the_default_units_per_context() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch("DROP TABLE units; PRAGMA user_version = 9;")
            .unwrap();
        let body = serde_json::to_string(&ContextBody {
            brand: "blue".into(),
            settings: settings(),
        })
        .unwrap();
        conn.execute("INSERT INTO contexts (json) VALUES (?1), (?1)", [body])
            .unwrap();

        let store = Store::init(conn).unwrap();

        let version: i32 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        for context in [1, 2] {
            assert_eq!(labels(&store, context), ["Stk.", "Std.", "pauschal"]);
        }

        // stamped back, it migrates again and nothing is added twice
        store
            .conn()
            .execute_batch("PRAGMA user_version = 9;")
            .unwrap();
        migrate(&store.conn()).unwrap();
        assert_eq!(labels(&store, 1).len(), 3);
        assert_eq!(labels(&store, 2).len(), 3);
    }

    #[test]
    fn a_context_starts_with_the_default_units() {
        let store = Store::in_memory().unwrap();

        let first = context(&store, "A");

        assert_eq!(
            store.units(first.id.unwrap()).unwrap().len(),
            units::DEFAULTS.len()
        );
        assert_eq!(
            labels(&store, first.id.unwrap()),
            ["Stk.", "Std.", "pauschal"]
        );
    }

    #[test]
    fn a_new_context_starts_with_the_units_it_is_given_or_the_defaults() {
        let store = Store::in_memory().unwrap();
        let given = [
            UnitDef {
                code: "DAY".into(),
                label: "Tag".into(),
            },
            UnitDef {
                code: "C62".into(),
                // stored trimmed, like a unit typed in
                label: " Lizenz ".into(),
            },
        ];
        let mut with_settings = settings();
        with_settings.seller.name = "B".into();

        let own = store
            .save_context(&Context::new(with_settings.clone(), ""), Some(&given), None)
            .unwrap();
        let empty = store
            .save_context(&Context::new(with_settings, ""), Some(&[]), None)
            .unwrap();

        assert_eq!(labels(&store, own.id.unwrap()), ["Tag", "Lizenz"]);
        assert_eq!(
            labels(&store, empty.id.unwrap()),
            ["Stk.", "Std.", "pauschal"]
        );
        // saving an existing context keeps its units
        store.save_context(&own, Some(&[]), None).unwrap();
        assert_eq!(labels(&store, own.id.unwrap()), ["Tag", "Lizenz"]);
    }

    #[test]
    fn units_that_are_handed_to_a_new_context_are_checked_and_nothing_is_made_when_they_fail() {
        let store = Store::in_memory().unwrap();
        let twice = [("C62", "Stk."), ("HUR", "stk.")].map(|(code, label)| UnitDef {
            code: code.into(),
            label: label.into(),
        });

        let refused = store.save_context(&Context::new(settings(), ""), Some(&twice), None);

        assert!(refused.is_err());
        assert!(store.contexts().unwrap().is_empty());
    }

    #[test]
    fn units_are_added_changed_and_checked() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let second = context(&store, "B").id.unwrap();

        let license = store
            .save_unit(&new_unit(first, "C62", "  Lizenz "))
            .unwrap();
        assert_eq!(license.label, "Lizenz");
        assert_eq!(license.context_id, first);
        // several labels per code, but a label only once per context, case aside
        assert!(store.save_unit(&new_unit(first, "HUR", "lizenz")).is_err());
        assert!(store.save_unit(&new_unit(second, "HUR", "lizenz")).is_ok());
        assert!(store.save_unit(&new_unit(first, "kwh", "kWh")).is_err());
        assert!(store.save_unit(&new_unit(first, "KWH", " ")).is_err());
        assert!(store.save_unit(&new_unit(9999, "KWH", "kWh")).is_err());

        // a change keeps the unit in its context and its place
        let renamed = store
            .save_unit(&Unit {
                label: "Software-Lizenz".into(),
                context_id: second,
                ..license
            })
            .unwrap();
        assert_eq!(renamed.context_id, first);
        assert_eq!(
            labels(&store, first),
            ["Stk.", "Std.", "pauschal", "Software-Lizenz"]
        );
        // it may keep its own label
        assert!(store.save_unit(&renamed).is_ok());
        assert!(store
            .save_unit(&Unit {
                label: "std.".into(),
                ..renamed
            })
            .is_err());
    }

    #[test]
    fn units_are_put_in_the_order_they_are_given() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let mut ids: Vec<i64> = store
            .units(first)
            .unwrap()
            .into_iter()
            .filter_map(|u| u.id)
            .collect();

        ids.reverse();
        store.reorder_units(first, &ids).unwrap();

        assert_eq!(labels(&store, first), ["pauschal", "Std.", "Stk."]);
        assert_eq!(labels(&store, other), ["Stk.", "Std.", "pauschal"]);
        // every unit of the context, once
        assert!(store.reorder_units(first, &ids[1..]).is_err());
        assert!(store
            .reorder_units(first, &[ids[0], ids[0], ids[1]])
            .is_err());
        let foreign = store.units(other).unwrap()[0].id.unwrap();
        assert!(store
            .reorder_units(first, &[ids[0], ids[1], foreign])
            .is_err());
    }

    #[test]
    fn renaming_a_unit_reaches_drafts_and_saved_positions_but_not_created_invoices() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let hours = unit_of(&store, first, "Std.");
        let scoped = saved_position(&store, Some(first), Some("HUR"), Some("Std."));
        let shared = saved_position(&store, None, Some("HUR"), None);
        let untouched = saved_position(&store, Some(first), Some("HUR"), Some("Stunde"));
        let elsewhere = draft_with_unit(&store, other, "HUR", "Std.");
        let open = draft_with_unit(&store, first, "HUR", "Std.");
        let issued = create_as(&store, first, Kind::Invoice, "RE-2026-001");
        let issued_before = stored_draft(&store, issued.id);
        let elsewhere_before = stored_draft(&store, elsewhere.id);
        let untouched_before = stored_template(&store, untouched.id.unwrap());
        let stamp = |id: i64| -> String {
            store
                .conn()
                .query_row("SELECT updated_at FROM invoices WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        let stamped = stamp(open.id);

        store
            .save_unit(&Unit {
                label: "Stunde(n)".into(),
                ..hours
            })
            .unwrap();

        let label_of = |p: &Position| p.unit_label.clone().unwrap();
        assert_eq!(
            label_of(&store.invoice(open.id).unwrap().draft.positions[0]),
            "Stunde(n)"
        );
        let templates = store.templates(first).unwrap();
        let find = |id: Option<i64>| templates.iter().find(|t| t.id == id).unwrap();
        assert_eq!(label_of(&find(scoped.id).position), "Stunde(n)");
        // a shared saved position follows, whichever context it is seen from
        assert_eq!(label_of(&find(shared.id).position), "Stunde(n)");
        // another label on the same code is another unit
        assert_eq!(
            stored_template(&store, untouched.id.unwrap()),
            untouched_before
        );
        // what belongs to another context or is created is not touched, and the lists keep their order
        assert_eq!(stored_draft(&store, elsewhere.id), elsewhere_before);
        assert_eq!(stored_draft(&store, issued.id), issued_before);
        assert_eq!(stamp(open.id), stamped);
    }

    #[test]
    fn changing_the_code_of_a_unit_reaches_drafts_too() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let hours = unit_of(&store, first, "Std.");
        let open = draft_with_unit(&store, first, "HUR", "Std.");

        store
            .save_unit(&Unit {
                code: "MIN".into(),
                ..hours
            })
            .unwrap();

        let position = &store.invoice(open.id).unwrap().draft.positions[0];
        assert_eq!(position.unit.as_deref(), Some("MIN"));
        assert_eq!(position.unit_label.as_deref(), Some("Std."));
    }

    #[test]
    fn what_a_unit_reaches_is_told_before_a_change() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let hours = unit_of(&store, first, "Std.");
        let piece = unit_of(&store, first, "Stk.");
        let template = saved_position(&store, None, Some("HUR"), Some("Std."));
        let open = draft_with_unit(&store, first, "HUR", "Std.");
        // not reached: created, and another context's draft
        create_as(&store, first, Kind::Invoice, "RE-2026-001");
        let other = context(&store, "B").id.unwrap();
        draft_with_unit(&store, other, "HUR", "Std.");

        let usage = store.unit_usage(hours.id.unwrap()).unwrap();

        assert_eq!(usage.templates.len(), 1);
        assert_eq!(usage.templates[0].id, template.id.unwrap());
        assert_eq!(usage.templates[0].context_id, None);
        assert_eq!(usage.drafts.len(), 1);
        assert_eq!(usage.drafts[0].invoice_id, open.id);
        assert_eq!(usage.drafts[0].customer, customer().name);
        let counts = store.unit_counts(first).unwrap();
        assert_eq!(counts[&hours.id.unwrap()], 2);
        assert_eq!(counts[&piece.id.unwrap()], 0);
    }

    #[test]
    fn a_unit_nothing_uses_is_deleted_and_the_last_one_stays() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let ids: Vec<i64> = store
            .units(first)
            .unwrap()
            .into_iter()
            .filter_map(|u| u.id)
            .collect();

        store.delete_unit(ids[2], None).unwrap();
        store.delete_unit(ids[1], None).unwrap();

        assert_eq!(labels(&store, first), ["Stk."]);
        assert!(store.delete_unit(ids[0], None).is_err());
        assert!(store.delete_unit(9999, None).is_err());
        assert_eq!(labels(&store, first), ["Stk."]);
    }

    #[test]
    fn a_unit_in_use_is_deleted_only_with_a_replacement_that_takes_its_place() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let hours = unit_of(&store, first, "Std.").id.unwrap();
        let piece = unit_of(&store, first, "Stk.");
        let foreign = unit_of(&store, other, "Stk.").id.unwrap();
        let template = saved_position(&store, Some(first), Some("HUR"), Some("Std."));
        let open = draft_with_unit(&store, first, "HUR", "Std.");
        let issued = create_as(&store, first, Kind::Invoice, "RE-2026-001");
        // archived, too
        let archived = create_as(&store, first, Kind::Invoice, "RE-2026-002");
        store.archive_invoice(archived.id).unwrap();
        let (issued_before, archived_before) = (
            stored_draft(&store, issued.id),
            stored_draft(&store, archived.id),
        );

        // used: a replacement is needed, of the same context and another unit
        assert!(store.delete_unit(hours, None).is_err());
        assert!(store.delete_unit(hours, Some(hours)).is_err());
        assert!(store.delete_unit(hours, Some(foreign)).is_err());
        assert_eq!(labels(&store, first).len(), 3);

        store.delete_unit(hours, Some(piece.id.unwrap())).unwrap();

        assert_eq!(labels(&store, first), ["Stk.", "pauschal"]);
        let position = &store.invoice(open.id).unwrap().draft.positions[0];
        assert_eq!(position.unit.as_deref(), Some("C62"));
        assert_eq!(position.unit_label.as_deref(), Some("Stk."));
        let kept = store
            .templates(first)
            .unwrap()
            .into_iter()
            .find(|t| t.id == template.id)
            .unwrap();
        assert_eq!(kept.position.unit_label.as_deref(), Some("Stk."));
        assert_eq!(stored_draft(&store, issued.id), issued_before);
        assert_eq!(stored_draft(&store, archived.id), archived_before);
    }

    #[test]
    fn removing_a_context_removes_its_units_with_it() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();

        store.remove_context(first, Disposition::Share).unwrap();

        assert!(store.units(first).unwrap().is_empty());
        assert_eq!(store.units(other).unwrap().len(), 3);
    }

    #[test]
    fn positions_are_read_with_code_and_label_filled_in() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        // what an older version stored: no label, and piece as an empty code
        store
            .conn()
            .execute(
                "INSERT INTO templates (json, context_id) VALUES
                     ('{\"description\":\"Alt\",\"quantity\":null,\"unit\":null,\"price\":\"1\"}', NULL),
                     ('{\"description\":\"Woche\",\"quantity\":null,\"unit\":\"WEE\",\"price\":\"1\"}', NULL)",
                [],
            )
            .unwrap();

        let templates = store.templates(first).unwrap();

        let read = |description: &str| {
            let t = templates
                .iter()
                .find(|t| t.position.description == description)
                .unwrap();
            (
                t.position.unit.clone().unwrap(),
                t.position.unit_label.clone().unwrap(),
            )
        };
        assert_eq!(read("Alt"), ("C62".to_owned(), "Stk.".to_owned()));
        assert_eq!(read("Woche"), ("WEE".to_owned(), "Woche(n)".to_owned()));
        // reading did not rewrite anything
        assert!(stored_template(&store, 1).contains("\"unit\":null"));
    }

    #[test]
    fn saved_positions_with_other_labels_on_one_code_are_different() {
        let store = Store::in_memory().unwrap();
        let first = saved_position(&store, None, Some("HUR"), Some("Std."));

        let other = saved_position(&store, None, Some("HUR"), Some("Stunde"));
        let same = saved_position(&store, None, Some("HUR"), None);

        assert_ne!(first.id, other.id);
        // no label is what the code was printed as
        assert_eq!(first.id, same.id);
    }

    // VAT rates

    fn rated(description: &str, rate: Option<&str>, label: Option<&str>) -> Position {
        Position {
            tax_rate: rate.map(Into::into),
            tax_label: label.map(Into::into),
            ..position(description, None, None)
        }
    }

    fn saved_rated(
        store: &Store,
        context: Option<i64>,
        rate: Option<&str>,
        label: Option<&str>,
    ) -> PositionTemplate {
        store
            .save_template(&PositionTemplate {
                id: None,
                context_id: context,
                position: rated("Leistung", rate, label),
            })
            .unwrap()
    }

    fn draft_with_rate(store: &Store, context: i64, rate: &str, label: &str) -> StoredInvoice {
        let mut named = draft();
        named.positions = vec![rated("Leistung", Some(rate), Some(label))];
        store
            .save_draft(None, context, Kind::Invoice, &named, None)
            .unwrap()
    }

    fn rate_of(store: &Store, context: i64, label: &str) -> Rate {
        store
            .rates(context)
            .unwrap()
            .into_iter()
            .find(|r| r.label == label)
            .unwrap()
    }

    /// The list of a context as label and rate
    fn rate_list(store: &Store, context: i64) -> Vec<(String, String)> {
        store
            .rates(context)
            .unwrap()
            .into_iter()
            .map(|r| (r.label, r.rate))
            .collect()
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(label, rate)| ((*label).to_owned(), (*rate).to_owned()))
            .collect()
    }

    fn new_rate(context: i64, label: &str, rate: &str) -> Rate {
        Rate {
            id: None,
            context_id: context,
            label: label.into(),
            rate: rate.into(),
        }
    }

    fn rate_def(label: &str, rate: &str) -> RateDef {
        RateDef {
            label: label.into(),
            rate: rate.into(),
        }
    }

    /// The rate the positions of a context take when they pick none
    fn default_of(store: &Store, context: i64) -> String {
        store.context(context).unwrap().settings.tax_rate
    }

    fn context_charging(store: &Store, seller: &str, rate: &str) -> i64 {
        let mut with_rate = settings();
        with_rate.seller.name = seller.into();
        with_rate.tax_rate = rate.into();
        store
            .save_context(&Context::new(with_rate, ""), None, None)
            .unwrap()
            .id
            .unwrap()
    }

    #[test]
    fn a_version_ten_database_gets_the_default_rates_and_the_one_it_charges_per_context() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch("DROP TABLE rates; PRAGMA user_version = 10;")
            .unwrap();
        // what a context charged before there was a list
        for charged in ["19", "16", "", "5,50", "7,0", "abc"] {
            let body = serde_json::to_string(&ContextBody {
                brand: "blue".into(),
                settings: Settings {
                    tax_rate: charged.into(),
                    ..settings()
                },
            })
            .unwrap();
            conn.execute("INSERT INTO contexts (json) VALUES (?1)", [body])
                .unwrap();
        }
        conn.execute(
            "INSERT INTO templates (json, context_id) VALUES ('{\"description\":\"Alt\",\"price\":\"1\"}', NULL)",
            [],
        )
        .unwrap();

        let store = Store::init(conn).unwrap();

        let version: i32 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let defaults = [("Regelsatz", "19"), ("Ermäßigt", "7")];
        let with = |extra: (&'static str, &'static str)| [&defaults[..], &[extra]].concat();
        assert_eq!(rate_list(&store, 1), pairs(&defaults));
        assert_eq!(rate_list(&store, 2), pairs(&with(("Steuersatz", "16"))));
        // no VAT is no rate, and nothing is made of what is no rate
        assert_eq!(rate_list(&store, 3), pairs(&defaults));
        assert_eq!(rate_list(&store, 4), pairs(&with(("Steuersatz", "5.5"))));
        assert_eq!(rate_list(&store, 5), pairs(&defaults));
        assert_eq!(rate_list(&store, 6), pairs(&defaults));
        // no conversion of anything else: the positions are as they were
        assert_eq!(
            stored_template(&store, 1),
            "{\"description\":\"Alt\",\"price\":\"1\"}"
        );
        assert_eq!(default_of(&store, 2), "16");

        // stamped back, it migrates again and nothing is added twice
        store
            .conn()
            .execute_batch("PRAGMA user_version = 10;")
            .unwrap();
        migrate(&store.conn()).unwrap();
        assert_eq!(store.rates(2).unwrap().len(), 3);
        assert_eq!(store.rates(1).unwrap().len(), 2);
    }

    #[test]
    fn a_context_starts_with_the_default_rates() {
        let store = Store::in_memory().unwrap();

        let first = context(&store, "A");

        assert_eq!(
            rate_list(&store, first.id.unwrap()),
            pairs(&[("Regelsatz", "19"), ("Ermäßigt", "7")])
        );
    }

    #[test]
    fn a_new_context_starts_with_the_rates_it_is_given_and_the_one_it_charges() {
        let store = Store::in_memory().unwrap();
        let given = [rate_def("Lieferung", "16"), rate_def(" Sonder ", "5,50")];
        let mut with_settings = settings();
        with_settings.seller.name = "B".into();

        let own = store
            .save_context(&Context::new(with_settings.clone(), ""), None, Some(&given))
            .unwrap();
        let empty = store
            .save_context(&Context::new(with_settings.clone(), ""), None, Some(&[]))
            .unwrap();
        with_settings.tax_rate.clear();
        let free = store
            .save_context(&Context::new(with_settings, ""), None, Some(&given))
            .unwrap();

        // stored trimmed and canonical, with the default (19) among them
        assert_eq!(
            rate_list(&store, own.id.unwrap()),
            pairs(&[("Lieferung", "16"), ("Sonder", "5.5"), ("Steuersatz", "19")])
        );
        assert_eq!(
            rate_list(&store, empty.id.unwrap()),
            pairs(&[("Regelsatz", "19"), ("Ermäßigt", "7")])
        );
        // no VAT: nothing is added
        assert_eq!(
            rate_list(&store, free.id.unwrap()),
            pairs(&[("Lieferung", "16"), ("Sonder", "5.5")])
        );
        // saving an existing context keeps its rates
        store.save_context(&own, None, Some(&[])).unwrap();
        assert_eq!(store.rates(own.id.unwrap()).unwrap().len(), 3);
    }

    #[test]
    fn a_name_for_the_default_that_is_taken_is_made_unique() {
        let store = Store::in_memory().unwrap();
        let given = [rate_def("steuersatz", "16")];

        let made = store
            .save_context(&Context::new(settings(), ""), None, Some(&given))
            .unwrap();

        assert_eq!(
            rate_list(&store, made.id.unwrap()),
            pairs(&[("steuersatz", "16"), ("Steuersatz 19 %", "19")])
        );
    }

    #[test]
    fn rates_that_are_handed_to_a_new_context_are_checked_and_nothing_is_made_when_they_fail() {
        let store = Store::in_memory().unwrap();

        for twice in [
            [rate_def("Regel", "19"), rate_def("regel", "7")],
            [rate_def("Regel", "19"), rate_def("Normal", "19,0")],
            [rate_def("Regel", "19"), rate_def("Null", "0")],
            [rate_def("Regel", "19"), rate_def(" ", "7")],
        ] {
            let refused = store.save_context(&Context::new(settings(), ""), None, Some(&twice));

            assert!(refused.is_err());
            assert!(store.contexts().unwrap().is_empty());
        }
    }

    #[test]
    fn rates_are_added_changed_and_checked() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let second = context(&store, "B").id.unwrap();

        let added = store
            .save_rate(&new_rate(first, "  Lieferung ", "16,0"))
            .unwrap();

        assert_eq!(
            (added.label.as_str(), added.rate.as_str()),
            ("Lieferung", "16")
        );
        assert_eq!(store.rates(first).unwrap().last(), Some(&added));
        // the label and the rate are unique per context, the label case aside
        assert!(store.save_rate(&new_rate(first, "lieferung", "3")).is_err());
        assert!(store.save_rate(&new_rate(first, "Andere", "16")).is_err());
        assert!(store
            .save_rate(&new_rate(first, "Andere", "19,00"))
            .is_err());
        assert!(store
            .save_rate(&new_rate(second, "lieferung", "16"))
            .is_ok());
        for bad in ["0", "-1", "100,5", "", "x"] {
            assert!(
                store.save_rate(&new_rate(first, "Neu", bad)).is_err(),
                "{bad}"
            );
        }
        assert!(store.save_rate(&new_rate(first, " ", "3")).is_err());
        assert!(store.save_rate(&new_rate(9999, "Neu", "3")).is_err());

        // a change keeps the rate in its context and its place
        let renamed = Rate {
            label: "Lieferung EU".into(),
            rate: "16,5".into(),
            ..added.clone()
        };
        let changed = store.save_rate(&renamed).unwrap();
        assert_eq!(changed.rate, "16.5");
        assert_eq!(store.rates(first).unwrap().last(), Some(&changed));
        // unchanged values are not their own duplicates
        assert!(store.save_rate(&changed).is_ok());
        let taken = Rate {
            label: "Regelsatz".into(),
            ..changed
        };
        assert!(store.save_rate(&taken).is_err());
    }

    #[test]
    fn rates_are_put_in_the_order_they_are_given() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let mut ids: Vec<i64> = store
            .rates(first)
            .unwrap()
            .into_iter()
            .filter_map(|r| r.id)
            .collect();
        ids.reverse();

        store.reorder_rates(first, &ids).unwrap();

        assert_eq!(
            rate_list(&store, first),
            pairs(&[("Ermäßigt", "7"), ("Regelsatz", "19")])
        );
        // every rate of the context, once
        assert!(store.reorder_rates(first, &ids[1..]).is_err());
        assert!(store.reorder_rates(first, &[ids[0], ids[0]]).is_err());
        let foreign = store.rates(other).unwrap()[0].id.unwrap();
        assert!(store.reorder_rates(first, &[ids[0], foreign]).is_err());
    }

    #[test]
    fn a_new_label_reaches_drafts_and_saved_positions_but_the_default_and_created_invoices_stay() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let regular = rate_of(&store, first, "Regelsatz");
        let template = saved_rated(&store, Some(first), Some("19"), Some("Regelsatz"));
        let open = draft_with_rate(&store, first, "19", "Regelsatz");
        let elsewhere = draft_with_rate(&store, other, "19", "Regelsatz");
        let issued = {
            let id = draft_with_rate(&store, first, "19", "Regelsatz").id;
            store
                .finalize(
                    id,
                    &at("RE-2026-001", 1),
                    &settings(),
                    None,
                    "1.00",
                    &files(),
                )
                .unwrap()
        };
        let issued_before = stored_draft(&store, issued.id);
        let elsewhere_before = stored_draft(&store, elsewhere.id);

        store
            .save_rate(&Rate {
                label: "Normalsatz".into(),
                ..regular
            })
            .unwrap();

        let position = &store.invoice(open.id).unwrap().draft.positions[0];
        assert_eq!(position.tax_rate.as_deref(), Some("19"));
        assert_eq!(position.tax_label.as_deref(), Some("Normalsatz"));
        let kept = store
            .templates(first)
            .unwrap()
            .into_iter()
            .find(|t| t.id == template.id)
            .unwrap();
        assert_eq!(kept.position.tax_label.as_deref(), Some("Normalsatz"));
        // the default is a rate without a name
        assert_eq!(default_of(&store, first), "19");
        assert_eq!(stored_draft(&store, issued.id), issued_before);
        assert_eq!(stored_draft(&store, elsewhere.id), elsewhere_before);
    }

    #[test]
    fn a_new_value_reaches_drafts_saved_positions_and_the_default_of_the_context() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let regular = rate_of(&store, first, "Regelsatz");
        let reduced = saved_rated(&store, None, Some("7"), Some("Ermäßigt"));
        let picked = saved_rated(&store, None, Some("19,0"), Some("Regelsatz"));
        let by_default = saved_rated(&store, None, None, None);
        let open = draft_with_rate(&store, first, "19", "Regelsatz");
        let elsewhere = draft_with_rate(&store, other, "19", "Regelsatz");
        let elsewhere_before = stored_draft(&store, elsewhere.id);

        store
            .save_rate(&Rate {
                rate: "20".into(),
                ..regular
            })
            .unwrap();

        let position = &store.invoice(open.id).unwrap().draft.positions[0];
        assert_eq!(position.tax_rate.as_deref(), Some("20"));
        assert_eq!(default_of(&store, first), "20");
        let templates = store.templates(first).unwrap();
        let of = |id: Option<i64>| templates.iter().find(|t| t.id == id).unwrap();
        assert_eq!(of(picked.id).position.tax_rate.as_deref(), Some("20"));
        // what picked another rate or none stays as it is
        assert_eq!(of(reduced.id).position.tax_rate.as_deref(), Some("7"));
        assert_eq!(of(by_default.id).position.tax_rate, None);
        // another context has its own list and its own default
        assert_eq!(stored_draft(&store, elsewhere.id), elsewhere_before);
        assert_eq!(default_of(&store, other), "19");
    }

    #[test]
    fn what_a_rate_reaches_is_told_before_a_change() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let regular = rate_of(&store, first, "Regelsatz");
        let reduced = rate_of(&store, first, "Ermäßigt");
        let template = saved_rated(&store, Some(first), Some("19"), Some("Regelsatz"));
        let open = draft_with_rate(&store, first, "19", "Regelsatz");
        draft_with_rate(&store, other, "19", "Regelsatz");

        let usage = store.rate_usage(regular.id.unwrap()).unwrap();

        assert_eq!(usage.templates.len(), 1);
        assert_eq!(usage.templates[0].id, template.id.unwrap());
        assert_eq!(usage.drafts.len(), 1);
        assert_eq!(usage.drafts[0].invoice_id, open.id);
        assert!(usage.default);
        let unused = store.rate_usage(reduced.id.unwrap()).unwrap();
        assert!(unused.is_empty() && !unused.default);
        let counts = store.rate_counts(first).unwrap();
        assert_eq!(counts[&regular.id.unwrap()], 2);
        assert_eq!(counts[&reduced.id.unwrap()], 0);
    }

    #[test]
    fn a_rate_nothing_uses_is_deleted() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let reduced = rate_of(&store, first, "Ermäßigt").id.unwrap();
        let regular = rate_of(&store, first, "Regelsatz").id.unwrap();

        store.delete_rate(reduced, None).unwrap();

        assert_eq!(rate_list(&store, first), pairs(&[("Regelsatz", "19")]));
        // it is the default of the context, so it is in use
        assert!(store.delete_rate(regular, None).is_err());
        assert!(store.delete_rate(9999, None).is_err());
        assert_eq!(store.rates(first).unwrap().len(), 1);
    }

    #[test]
    fn a_context_without_vat_can_have_no_rates_at_all() {
        let store = Store::in_memory().unwrap();
        let free = context_charging(&store, "A", "");

        store
            .rates(free)
            .unwrap()
            .into_iter()
            .for_each(|r| store.delete_rate(r.id.unwrap(), None).unwrap());

        assert!(store.rates(free).unwrap().is_empty());
        assert_eq!(default_of(&store, free), "");
    }

    #[test]
    fn a_rate_in_use_is_deleted_only_with_a_replacement_that_takes_its_place() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        let regular = rate_of(&store, first, "Regelsatz").id.unwrap();
        let reduced = rate_of(&store, first, "Ermäßigt");
        let foreign = rate_of(&store, other, "Ermäßigt").id.unwrap();
        let template = saved_rated(&store, Some(first), Some("19"), Some("Regelsatz"));
        let open = draft_with_rate(&store, first, "19", "Regelsatz");
        let issued = {
            let id = draft_with_rate(&store, first, "19", "Regelsatz").id;
            store
                .finalize(
                    id,
                    &at("RE-2026-001", 1),
                    &settings(),
                    None,
                    "1.00",
                    &files(),
                )
                .unwrap()
        };
        let issued_before = stored_draft(&store, issued.id);

        // used: a replacement is needed, of the same context and another rate
        assert!(store.delete_rate(regular, None).is_err());
        assert!(store.delete_rate(regular, Some(regular)).is_err());
        assert!(store.delete_rate(regular, Some(foreign)).is_err());
        assert_eq!(store.rates(first).unwrap().len(), 2);

        store
            .delete_rate(regular, Some(reduced.id.unwrap()))
            .unwrap();

        assert_eq!(rate_list(&store, first), pairs(&[("Ermäßigt", "7")]));
        let position = &store.invoice(open.id).unwrap().draft.positions[0];
        assert_eq!(position.tax_rate.as_deref(), Some("7"));
        assert_eq!(position.tax_label.as_deref(), Some("Ermäßigt"));
        let kept = store
            .templates(first)
            .unwrap()
            .into_iter()
            .find(|t| t.id == template.id)
            .unwrap();
        assert_eq!(kept.position.tax_label.as_deref(), Some("Ermäßigt"));
        // the default was the deleted rate, the replacement is the default now
        assert_eq!(default_of(&store, first), "7");
        assert_eq!(stored_draft(&store, issued.id), issued_before);
    }

    #[test]
    fn removing_a_context_removes_its_rates_with_it() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();

        store.remove_context(first, Disposition::Share).unwrap();

        assert!(store.rates(first).unwrap().is_empty());
        assert_eq!(store.rates(other).unwrap().len(), 2);
    }

    #[test]
    fn saved_positions_with_another_rate_are_different() {
        let store = Store::in_memory().unwrap();
        let first = saved_rated(&store, None, Some("7"), Some("Ermäßigt"));

        let other_rate = saved_rated(&store, None, Some("19"), Some("Regelsatz"));
        let default = saved_rated(&store, None, None, None);
        let same = saved_rated(&store, None, Some("7,0"), Some("Ermäßigt"));
        let other_label = saved_rated(&store, None, Some("7"), Some("Essen"));

        assert_ne!(first.id, other_rate.id);
        assert_ne!(first.id, default.id);
        assert_ne!(first.id, other_label.id);
        assert_eq!(first.id, same.id);
    }

    // the defaults of the lists

    /// The label of the unit new positions of a context start with, as stored
    fn unit_setting(store: &Store, context: i64) -> Option<String> {
        store.context(context).unwrap().settings.unit
    }

    #[test]
    fn a_context_starts_without_a_default_unit_and_one_is_chosen_from_its_list() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        assert_eq!(unit_setting(&store, first), None);

        store.choose_default_unit(first, Some("Std.")).unwrap();

        assert_eq!(unit_setting(&store, first).as_deref(), Some("Std."));
        // another context has its own
        assert_eq!(unit_setting(&store, other), None);
        // only a unit of the list, none puts the first one back
        assert!(store.choose_default_unit(first, Some("Meter")).is_err());
        assert!(store.choose_default_unit(first, Some("std.")).is_err());
        assert!(store.choose_default_unit(9999, None).is_err());
        assert_eq!(unit_setting(&store, first).as_deref(), Some("Std."));
        store.choose_default_unit(first, None).unwrap();
        assert_eq!(unit_setting(&store, first), None);
    }

    #[test]
    fn renaming_the_default_unit_moves_the_setting_with_it() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let hours = unit_of(&store, first, "Std.");
        let piece = unit_of(&store, first, "Stk.");
        store.choose_default_unit(first, Some("Std.")).unwrap();

        // another unit changes: the default stays
        store
            .save_unit(&Unit {
                label: "Stück".into(),
                ..piece
            })
            .unwrap();
        assert_eq!(unit_setting(&store, first).as_deref(), Some("Std."));

        store
            .save_unit(&Unit {
                label: " Stunde ".into(),
                ..hours.clone()
            })
            .unwrap();
        assert_eq!(unit_setting(&store, first).as_deref(), Some("Stunde"));

        // a change of its code only keeps the label, and the setting
        store
            .save_unit(&Unit {
                code: "MIN".into(),
                label: "Stunde".into(),
                ..hours
            })
            .unwrap();
        assert_eq!(unit_setting(&store, first).as_deref(), Some("Stunde"));
    }

    #[test]
    fn deleting_the_default_unit_puts_the_first_one_back_with_or_without_a_replacement() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let hours = unit_of(&store, first, "Std.").id.unwrap();
        let flat = unit_of(&store, first, "pauschal");
        let piece = unit_of(&store, first, "Stk.");
        let other = context(&store, "B").id.unwrap();
        store.choose_default_unit(first, Some("Std.")).unwrap();
        store.choose_default_unit(other, Some("Std.")).unwrap();

        store.delete_unit(hours, None).unwrap();

        assert_eq!(unit_setting(&store, first), None);
        // the other context has a unit of that label of its own
        assert_eq!(unit_setting(&store, other).as_deref(), Some("Std."));

        // one that is used by a draft needs a replacement, the default is cleared as well
        store.choose_default_unit(first, Some("pauschal")).unwrap();
        draft_with_unit(&store, first, "LS", "pauschal");
        store.delete_unit(flat.id.unwrap(), piece.id).unwrap();
        assert_eq!(unit_setting(&store, first), None);
    }

    #[test]
    fn the_default_rate_is_chosen_from_the_list_or_no_vat() {
        let store = Store::in_memory().unwrap();
        let first = context(&store, "A").id.unwrap();
        let other = context(&store, "B").id.unwrap();
        assert_eq!(default_of(&store, first), "19");

        store.choose_default_rate(first, "7,0").unwrap();

        // stored the way the list has it
        assert_eq!(default_of(&store, first), "7");
        assert_eq!(default_of(&store, other), "19");
        store.choose_default_rate(first, " ").unwrap();
        assert_eq!(default_of(&store, first), "");
        assert!(store.context(first).unwrap().settings.vat_free());
        // only a rate of the list
        assert!(store.choose_default_rate(first, "16").is_err());
        assert!(store.choose_default_rate(first, "abc").is_err());
        assert!(store.choose_default_rate(9999, "19").is_err());
        assert_eq!(default_of(&store, first), "");
        store.choose_default_rate(first, "19").unwrap();
        assert_eq!(default_of(&store, first), "19");
    }
}
