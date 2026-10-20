//! SQLite storage: settings, customers and invoices. One file in the app data directory.
//!
//! Invoice numbers are assigned when an invoice is exported, never for drafts, and an exported
//! invoice cannot be changed or deleted any more (it can be duplicated).

use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use en16931::Date;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::model::{
    CustomLayout, Customer, InvoiceDraft, LayoutUsage, ListEntry, PositionTemplate, Settings,
    Status, StoredInvoice,
};
use crate::numbering::Pattern;

pub struct Store(Mutex<Connection>);

const SCHEMA_VERSION: i32 = 4;

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
        ..layout.clone()
    })?;
    match layout.id {
        Some(id) => {
            let changed = conn.execute(
                "UPDATE layouts SET json = ?1 WHERE id = ?2",
                params![json, id],
            )?;
            if changed == 0 {
                bail!("unknown layout");
            }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO layouts (json) VALUES (?1)", [json])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

fn stored(row: &Row) -> rusqlite::Result<(StoredInvoice, String)> {
    let draft: String = row.get("draft")?;
    let status: String = row.get("status")?;
    Ok((
        StoredInvoice {
            id: row.get("id")?,
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
        },
        draft,
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
        self.0.lock().expect("database lock")
    }

    // settings

    pub fn settings(&self) -> Result<Settings> {
        let json: Option<String> = self
            .conn()
            .query_row("SELECT json FROM settings WHERE id = 1", [], |r| r.get(0))
            .optional()?;
        Ok(json
            .map(|j| serde_json::from_str::<Settings>(&j))
            .transpose()?
            .unwrap_or_default()
            .normalized())
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            [serde_json::to_string(settings)?],
        )?;
        Ok(())
    }

    // customers

    pub fn customers(&self) -> Result<Vec<Customer>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id, json FROM customers")?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut customers = rows
            .map(|row| {
                let (id, json) = row?;
                let mut customer: Customer = serde_json::from_str(&json)?;
                customer.id = Some(id);
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
            ..customer.clone()
        })?;
        let id = match customer.id {
            Some(id) => {
                let changed = conn.execute(
                    "UPDATE customers SET json = ?1 WHERE id = ?2",
                    params![json, id],
                )?;
                if changed == 0 {
                    bail!("unknown customer");
                }
                id
            }
            None => {
                conn.execute("INSERT INTO customers (json) VALUES (?1)", [json])?;
                conn.last_insert_rowid()
            }
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

    pub fn templates(&self) -> Result<Vec<PositionTemplate>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id, json FROM templates")?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut templates = rows
            .map(|row| {
                let (id, json) = row?;
                let mut template: PositionTemplate = serde_json::from_str(&json)?;
                template.id = Some(id);
                Ok(template)
            })
            .collect::<Result<Vec<_>>>()?;
        templates.sort_by_key(|t| t.position.description.to_lowercase());
        Ok(templates)
    }

    /// Saves a template. A new one that is the same as a saved one is not saved twice, the saved
    /// one comes back instead.
    pub fn save_template(&self, template: &PositionTemplate) -> Result<PositionTemplate> {
        if template.id.is_none() {
            if let Some(same) = self
                .templates()?
                .into_iter()
                .find(|t| t.position.same_as(&template.position))
            {
                return Ok(same);
            }
        }
        let conn = self.conn();
        let json = serde_json::to_string(&PositionTemplate {
            id: None,
            ..template.clone()
        })?;
        let id = match template.id {
            Some(id) => {
                let changed = conn.execute(
                    "UPDATE templates SET json = ?1 WHERE id = ?2",
                    params![json, id],
                )?;
                if changed == 0 {
                    bail!("unknown template");
                }
                id
            }
            None => {
                conn.execute("INSERT INTO templates (json) VALUES (?1)", [json])?;
                conn.last_insert_rowid()
            }
        };
        Ok(PositionTemplate {
            id: Some(id),
            ..template.clone()
        })
    }

    pub fn delete_template(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM templates WHERE id = ?1", [id])?;
        Ok(())
    }

    // own layouts

    /// Own layouts by name. One that has no `uid` yet (made before there were any) gets it here, once.
    pub fn custom_layouts(&self) -> Result<Vec<CustomLayout>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id, json FROM layouts")?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut layouts = rows
            .map(|row| {
                let (id, json) = row?;
                let mut layout: CustomLayout = serde_json::from_str(&json)?;
                layout.id = Some(id);
                Ok(layout)
            })
            .collect::<Result<Vec<_>>>()?;
        drop(statement);
        for layout in layouts.iter_mut().filter(|l| l.uid.is_empty()) {
            layout.uid = new_uid();
            layout.updated_at = now();
            put(&conn, layout)?;
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
        let mut layouts = self.custom_layouts()?;
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT draft FROM invoices")?;
        for json in statement.query_map([], |r| r.get::<_, String>(0))? {
            let draft: InvoiceDraft = serde_json::from_str(&json?)?;
            layouts.extend(draft.frozen_layout);
        }
        Ok(layouts)
    }

    /// The invoices (drafts and exported ones) made with the own layout: chosen for themselves, or
    /// frozen into them on export.
    pub fn layout_usages(&self, id: i64) -> Result<Vec<LayoutUsage>> {
        let key = format!("custom-{id}");
        let conn = self.conn();
        let mut statement =
            conn.prepare("SELECT id, number, draft FROM invoices ORDER BY updated_at DESC")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut usages = Vec::new();
        for row in rows {
            let (invoice_id, number, json) = row?;
            let draft: InvoiceDraft = serde_json::from_str(&json)?;
            let frozen = draft.frozen_layout.as_ref().and_then(|l| l.id) == Some(id);
            if draft.layout.as_deref() == Some(key.as_str()) || frozen {
                usages.push(LayoutUsage {
                    invoice_id,
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

    pub fn invoices(&self) -> Result<Vec<ListEntry>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, number, status, draft, total, exported_at, paid_at, updated_at FROM invoices
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], stored)?;
        rows.map(|row| {
            let (invoice, draft) = row?;
            let draft: InvoiceDraft = serde_json::from_str(&draft)?;
            Ok(ListEntry {
                id: invoice.id,
                number: invoice.number,
                status: invoice.status,
                customer_name: draft.customer.name,
                issue_date: draft.issue_date,
                total: invoice.total,
                paid_at: invoice.paid_at,
                updated_at: invoice.updated_at,
            })
        })
        .collect()
    }

    pub fn invoice(&self, id: i64) -> Result<StoredInvoice> {
        let conn = self.conn();
        let (mut invoice, draft) = conn
            .query_row(
                "SELECT id, number, status, draft, total, exported_at, paid_at, updated_at FROM invoices WHERE id = ?1",
                [id],
                stored,
            )
            .optional()?
            .ok_or_else(|| anyhow!("unknown invoice"))?;
        invoice.draft = serde_json::from_str(&draft)?;
        Ok(invoice)
    }

    /// Marks an exported invoice as paid on a date, or as open again (`None`). The invoice itself stays
    /// locked, this is the one thing that happens to it afterwards.
    pub fn set_paid(&self, id: i64, paid_at: Option<&str>) -> Result<StoredInvoice> {
        if let Some(date) = paid_at {
            en16931::Date::parse(date).map_err(|_| anyhow!("the date of payment is not a date"))?;
        }
        let changed = self.conn().execute(
            "UPDATE invoices SET paid_at = ?1 WHERE id = ?2 AND status = 'exported'",
            params![paid_at, id],
        )?;
        if changed == 0 {
            bail!("only an exported invoice can be paid");
        }
        self.invoice(id)
    }

    /// Creates a draft (`id` is `None`) or updates one, exported invoices are refused.
    pub fn save_draft(
        &self,
        id: Option<i64>,
        draft: &InvoiceDraft,
        total: Option<&str>,
    ) -> Result<StoredInvoice> {
        let json = serde_json::to_string(draft)?;
        let id = {
            let conn = self.conn();
            match id {
                Some(id) => {
                    let changed = conn.execute(
                        "UPDATE invoices SET draft = ?1, total = ?2, updated_at = datetime('now')
                         WHERE id = ?3 AND status = 'draft'",
                        params![json, total, id],
                    )?;
                    if changed == 0 {
                        bail!("an exported invoice cannot be changed, duplicate it instead");
                    }
                    id
                }
                None => {
                    conn.execute(
                        "INSERT INTO invoices (status, draft, total, updated_at)
                         VALUES ('draft', ?1, ?2, datetime('now'))",
                        params![json, total],
                    )?;
                    conn.last_insert_rowid()
                }
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

    /// A new draft with the content of an existing invoice, dated `today`.
    pub fn duplicate(&self, id: i64, today: &str) -> Result<StoredInvoice> {
        let mut draft = self.invoice(id)?.draft;
        draft.issue_date = today.into();
        draft.delivery_date = today.into();
        draft.frozen = None;
        self.save_draft(None, &draft, None)
    }

    /// The next free number for an invoice of that day, by the pattern (see `numbering`).
    pub fn next_number(&self, pattern: &str, date: Date) -> Result<String> {
        let pattern = Pattern::parse(pattern)?;
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT number FROM invoices WHERE number IS NOT NULL")?;
        let existing = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(pattern.next(date, existing.iter().map(String::as_str)))
    }

    /// Makes a draft final: it gets its number, freezes the settings and can no longer change.
    /// Exporting an already final invoice again is fine and does not come through here.
    pub fn finalize(
        &self,
        id: i64,
        number: &str,
        frozen: &Settings,
        frozen_layout: Option<&CustomLayout>,
        total: &str,
    ) -> Result<StoredInvoice> {
        let mut draft = self.invoice(id)?.draft;
        draft.frozen = Some(frozen.clone());
        draft.frozen_layout = frozen_layout.cloned();
        let changed = self.conn().execute(
            "UPDATE invoices SET number = ?1, status = 'exported', draft = ?2, total = ?3,
                 exported_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?4 AND status = 'draft'",
            params![number, serde_json::to_string(&draft)?, total, id],
        );
        match changed {
            Ok(1) => self.invoice(id),
            Ok(_) => bail!("the invoice is already exported"),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                bail!("the number {number} is already taken")
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::fixtures::{customer, draft, settings};
    use crate::model::Position;

    #[test]
    fn settings_default_until_saved() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.settings().unwrap().tax_rate, "19");

        let mut changed = settings();
        changed.payment_days = Some(30);
        store.save_settings(&changed).unwrap();

        assert_eq!(store.settings().unwrap(), changed);
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
                .customers()
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
        assert_eq!(store.customers().unwrap().last().unwrap().name, "Charlie");

        store.delete_customer(a.id.unwrap()).unwrap();
        assert_eq!(store.customers().unwrap().len(), 1);
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
        assert_eq!(store.templates().unwrap().len(), 1);

        // a different price is another template
        store.save_template(&template("Beratung", "120")).unwrap();
        assert_eq!(store.templates().unwrap().len(), 2);
    }

    #[test]
    fn templates_are_created_updated_sorted_and_deleted() {
        let store = Store::in_memory().unwrap();
        let template = |description: &str| PositionTemplate {
            id: None,
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
                .templates()
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

        assert!(store.templates().unwrap().is_empty());
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
                .custom_layouts()
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
            .custom_layouts()
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
        assert!(store.custom_layouts().unwrap().iter().any(|l| l.archived));

        store.delete_custom_layout(b.id.unwrap()).unwrap();
        assert_eq!(store.custom_layouts().unwrap().len(), 1);
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
        store.save_draft(None, &chosen, None).unwrap();
        assert_eq!(store.layout_usages(id).unwrap().len(), 1);

        let exported = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(exported, "RE-2026-001", &settings(), Some(&layout), "1.00")
            .unwrap();
        assert_eq!(store.layout_usages(id).unwrap().len(), 2);
    }

    #[test]
    fn an_exported_invoice_can_be_marked_paid_and_open_again() {
        let store = Store::in_memory().unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;
        assert!(
            store.set_paid(id, Some("2026-10-20")).is_err(),
            "drafts are not paid"
        );

        store
            .finalize(id, "RE-2026-001", &settings(), None, "1.00")
            .unwrap();
        assert!(
            store.set_paid(id, Some("20.10.2026")).is_err(),
            "not an ISO date"
        );

        let paid = store.set_paid(id, Some("2026-10-20")).unwrap();
        assert_eq!(paid.paid_at.as_deref(), Some("2026-10-20"));
        assert_eq!(store.invoices().unwrap()[0].paid_at, paid.paid_at);

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
        let first = store.save_draft(None, &draft(), Some("1181.08")).unwrap();
        assert_eq!(first.status, Status::Draft);
        assert_eq!(first.number, None);

        let mut changed = draft();
        changed.note = Some("Danke".into());
        let updated = store
            .save_draft(Some(first.id), &changed, Some("2.00"))
            .unwrap();

        assert_eq!(updated.draft.note.as_deref(), Some("Danke"));
        let list = store.invoices().unwrap();
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
        let pattern = crate::numbering::DEFAULT_PATTERN;
        assert_eq!(
            store.next_number(pattern, day(2026, 3, 7)).unwrap(),
            "RE-2026-001"
        );

        let id = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(id, "RE-2026-001", &settings(), None, "1.00")
            .unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(id, "RE-2026-007", &settings(), None, "1.00")
            .unwrap();

        assert_eq!(
            store.next_number(pattern, day(2026, 3, 7)).unwrap(),
            "RE-2026-008"
        );
        assert_eq!(
            store.next_number(pattern, day(2027, 1, 1)).unwrap(),
            "RE-2027-001"
        );
        assert_eq!(
            store.next_number("[R-]YYYY-NNN", day(2026, 1, 1)).unwrap(),
            "R-2026-001"
        );
        assert_eq!(
            store.next_number("[X-]YYYY-NNN", day(2026, 1, 1)).unwrap(),
            "X-2026-001"
        );
        assert!(store.next_number("YYYY", day(2026, 1, 1)).is_err());
    }

    #[test]
    fn exported_invoices_are_locked_and_keep_their_settings() {
        let store = Store::in_memory().unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;

        let done = store
            .finalize(id, "RE-2026-001", &settings(), None, "1181.08")
            .unwrap();

        assert_eq!(done.status, Status::Exported);
        assert_eq!(done.number.as_deref(), Some("RE-2026-001"));
        assert_eq!(done.draft.frozen, Some(settings()));
        assert!(done.exported_at.is_some());
        assert!(store.save_draft(Some(id), &draft(), None).is_err());
        assert!(store.delete_draft(id).is_err());
        assert!(store
            .finalize(id, "RE-2026-002", &settings(), None, "1.00")
            .is_err());
    }

    #[test]
    fn a_number_is_never_given_twice() {
        let store = Store::in_memory().unwrap();
        let a = store.save_draft(None, &draft(), None).unwrap().id;
        let b = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(a, "RE-2026-001", &settings(), None, "1.00")
            .unwrap();

        let error = store
            .finalize(b, "RE-2026-001", &settings(), None, "1.00")
            .unwrap_err();

        assert!(error.to_string().contains("already taken"));
        assert_eq!(store.invoice(b).unwrap().status, Status::Draft);
    }

    #[test]
    fn duplicates_are_fresh_drafts_with_todays_date() {
        let store = Store::in_memory().unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;
        let done = store
            .finalize(id, "RE-2026-001", &settings(), None, "1181.08")
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
        let id = store.save_draft(None, &draft(), None).unwrap().id;

        store.delete_draft(id).unwrap();

        assert!(store.invoices().unwrap().is_empty());
        assert!(store.delete_draft(id).is_err());
    }
}
