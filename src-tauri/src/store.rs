//! SQLite storage: settings, customers and invoices. One file in the app data directory.
//!
//! Invoice numbers are assigned when an invoice is exported, never for drafts, and an exported
//! invoice cannot be changed or deleted any more (it can be duplicated).

use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::model::{Customer, InvoiceDraft, ListEntry, Settings, Status, StoredInvoice};

pub struct Store(Mutex<Connection>);

const SCHEMA_VERSION: i32 = 1;

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
    Ok(())
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
            .map(|j| serde_json::from_str(&j))
            .transpose()?
            .unwrap_or_default())
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

    // invoices

    pub fn invoices(&self) -> Result<Vec<ListEntry>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, number, status, draft, total, exported_at, updated_at FROM invoices
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
                updated_at: invoice.updated_at,
            })
        })
        .collect()
    }

    pub fn invoice(&self, id: i64) -> Result<StoredInvoice> {
        let conn = self.conn();
        let (mut invoice, draft) = conn
            .query_row(
                "SELECT id, number, status, draft, total, exported_at, updated_at FROM invoices WHERE id = ?1",
                [id],
                stored,
            )
            .optional()?
            .ok_or_else(|| anyhow!("unknown invoice"))?;
        invoice.draft = serde_json::from_str(&draft)?;
        Ok(invoice)
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

    /// The next free number for the year, e.g. `RE-2026-004`.
    pub fn next_number(&self, prefix: &str, year: i32) -> Result<String> {
        let start = format!("{prefix}{year}-");
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT number FROM invoices WHERE number IS NOT NULL")?;
        let highest = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(Result::ok)
            .filter_map(|n| n.strip_prefix(&start).and_then(|s| s.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
        Ok(format!("{start}{:03}", highest + 1))
    }

    /// Makes a draft final: it gets its number, freezes the settings and can no longer change.
    /// Exporting an already final invoice again is fine and does not come through here.
    pub fn finalize(
        &self,
        id: i64,
        number: &str,
        frozen: &Settings,
        total: &str,
    ) -> Result<StoredInvoice> {
        let mut draft = self.invoice(id)?.draft;
        draft.frozen = Some(frozen.clone());
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

    #[test]
    fn settings_default_until_saved() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.settings().unwrap().tax_rate, "19");

        let mut changed = settings();
        changed.payment_days = 30;
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
    fn numbers_count_up_per_year_and_prefix() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.next_number("RE-", 2026).unwrap(), "RE-2026-001");

        let id = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(id, "RE-2026-001", &settings(), "1.00")
            .unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(id, "RE-2026-007", &settings(), "1.00")
            .unwrap();

        assert_eq!(store.next_number("RE-", 2026).unwrap(), "RE-2026-008");
        assert_eq!(store.next_number("RE-", 2027).unwrap(), "RE-2027-001");
        assert_eq!(store.next_number("R-", 2026).unwrap(), "R-2026-001");
    }

    #[test]
    fn exported_invoices_are_locked_and_keep_their_settings() {
        let store = Store::in_memory().unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;

        let done = store
            .finalize(id, "RE-2026-001", &settings(), "1181.08")
            .unwrap();

        assert_eq!(done.status, Status::Exported);
        assert_eq!(done.number.as_deref(), Some("RE-2026-001"));
        assert_eq!(done.draft.frozen, Some(settings()));
        assert!(done.exported_at.is_some());
        assert!(store.save_draft(Some(id), &draft(), None).is_err());
        assert!(store.delete_draft(id).is_err());
        assert!(store
            .finalize(id, "RE-2026-002", &settings(), "1.00")
            .is_err());
    }

    #[test]
    fn a_number_is_never_given_twice() {
        let store = Store::in_memory().unwrap();
        let a = store.save_draft(None, &draft(), None).unwrap().id;
        let b = store.save_draft(None, &draft(), None).unwrap().id;
        store
            .finalize(a, "RE-2026-001", &settings(), "1.00")
            .unwrap();

        let error = store
            .finalize(b, "RE-2026-001", &settings(), "1.00")
            .unwrap_err();

        assert!(error.to_string().contains("already taken"));
        assert_eq!(store.invoice(b).unwrap().status, Status::Draft);
    }

    #[test]
    fn duplicates_are_fresh_drafts_with_todays_date() {
        let store = Store::in_memory().unwrap();
        let id = store.save_draft(None, &draft(), None).unwrap().id;
        let done = store
            .finalize(id, "RE-2026-001", &settings(), "1181.08")
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
