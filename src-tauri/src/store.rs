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
use crate::model::{
    CustomLayout, Customer, InvoiceDraft, InvoiceFiles, Kind, LayoutUsage, ListEntry,
    PositionTemplate, Settings, Status, StoredInvoice,
};
use crate::numbering::{protect_new_tokens, Calendar, Given, Pattern};

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

const SCHEMA_VERSION: i32 = 8;

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

fn new_uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// UTC, in a form that sorts like time does
fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
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

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn =
            Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
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
    pub fn save_context(&self, context: &Context) -> Result<Context> {
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
            let conn = self.conn();
            conn.execute("INSERT INTO contexts (json) VALUES (?1)", [json])?;
            conn.last_insert_rowid()
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
            Ok(template)
        })
        .collect()
    }

    pub fn delete_template(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM templates WHERE id = ?1", [id])?;
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
                 updated_at, archived_at, files
             FROM invoices WHERE (?2 OR archived_at IS NULL) AND (?1 IS NULL OR context_id = ?1)
                 AND kind = ?3
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![context, with_archived, kind.as_str()], stored)?;
        rows.map(|row| {
            let (invoice, draft, _) = row?;
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

    /// Drafts only: exported invoices stay, their numbers must not disappear.
    pub fn delete_draft(&self, id: i64) -> Result<()> {
        let changed = self.conn().execute(
            "DELETE FROM invoices WHERE id = ?1 AND status = 'draft'",
            [id],
        )?;
        if changed == 0 {
            bail!("only drafts can be deleted");
        }
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
        self.save_draft(None, source.context_id, source.kind, &draft, None)
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
        store.save_context(&Context::new(settings, "")).unwrap()
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
        let saved = store.save_context(&changed).unwrap();

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
        let a = store.save_context(&Context::new(with_layout, "")).unwrap();
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
        .unwrap();
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
            .save_context(&Context::new(with_offer_layout, ""))
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
}
