//! What happened to a document: an append-only list of events (`document_events`), the payments
//! received on an invoice (`payments`) and the answer to an offer (`invoices.answer`).
//!
//! Events are never changed or removed while their document lives, undoing a mark is an event of
//! its own. Edits are no events (drafts save themselves); what changed between two versions is read
//! from the snapshots kept with the draft exports and the baking. A version of a document is the
//! number of its draft exports, it is counted, never stored.

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context as _, Result};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

use crate::model::{
    Answer, Event, EventDetail, EventKind, InvoiceDraft, Kind, Payment, Status, StoredInvoice,
};
use crate::store::{now, Store};

/// Events, payments and answers. What older versions stored is turned into events where it has a
/// date (baked, archived), a paid date becomes one payment of the whole total; the column goes.
pub(crate) fn migrate_to_history(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS document_events (
             id INTEGER PRIMARY KEY,
             invoice_id INTEGER NOT NULL,
             kind TEXT NOT NULL,
             at TEXT NOT NULL,
             on_date TEXT,
             detail TEXT NOT NULL DEFAULT '{}',
             snapshot TEXT
         );
         CREATE INDEX IF NOT EXISTS document_events_invoice ON document_events (invoice_id);
         CREATE TABLE IF NOT EXISTS payments (
             id INTEGER PRIMARY KEY,
             invoice_id INTEGER NOT NULL,
             amount TEXT NOT NULL,
             paid_on TEXT NOT NULL,
             created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS payments_invoice ON payments (invoice_id);",
    )?;
    for (column, kind) in [("answer", "TEXT"), ("answered_on", "TEXT")] {
        if !crate::store::has_column(&tx, "invoices", column)? {
            tx.execute_batch(&format!("ALTER TABLE invoices ADD COLUMN {column} {kind};"))?;
        }
    }
    let has_paid = crate::store::has_column(&tx, "invoices", "paid_at")?;
    let paid = if has_paid { "paid_at" } else { "NULL" };
    let rows = {
        let mut statement = tx.prepare(&format!(
            "SELECT id, number, status, draft, total, exported_at, updated_at, issued_on,
                 archived_at, {paid}
             FROM invoices ORDER BY id"
        ))?;
        let rows = statement.query_map([], |r| {
            Ok(Backfill {
                id: r.get(0)?,
                number: r.get(1)?,
                exported: r.get::<_, String>(2)? == "exported",
                draft: r.get(3)?,
                total: r.get(4)?,
                exported_at: r.get(5)?,
                updated_at: r.get(6)?,
                issued_on: r.get(7)?,
                archived_at: r.get(8)?,
                paid_at: r.get(9)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for row in rows {
        if row.exported {
            // what a created document holds is what it was baked with
            let draft: InvoiceDraft = serde_json::from_str(&row.draft)?;
            let at = moment(row.exported_at.as_deref().unwrap_or(&row.updated_at));
            insert_event(
                &tx,
                row.id,
                EventKind::Baked,
                &at,
                row.issued_on.as_deref(),
                &EventDetail {
                    number: row.number.clone(),
                    ..EventDetail::default()
                },
                Some(&draft),
            )?;
        }
        if let Some(paid_on) = &row.paid_at {
            let amount = row.total.clone().unwrap_or_else(|| "0".into());
            tx.execute(
                "INSERT INTO payments (invoice_id, amount, paid_on, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![row.id, amount, paid_on, now()],
            )?;
            insert_event(
                &tx,
                row.id,
                EventKind::Payment,
                &format!("{paid_on}T12:00:00Z"),
                Some(paid_on),
                &EventDetail {
                    amount: Some(amount),
                    ..EventDetail::default()
                },
                None,
            )?;
        }
        if let Some(at) = &row.archived_at {
            let detail = EventDetail::default();
            let at = moment(at);
            insert_event(&tx, row.id, EventKind::Archived, &at, None, &detail, None)?;
        }
    }
    if has_paid {
        tx.execute_batch("ALTER TABLE invoices DROP COLUMN paid_at;")?;
    }
    tx.execute_batch("PRAGMA user_version = 12;")?;
    tx.commit()?;
    Ok(())
}

/// A stored time as events keep it (`now()`: `2026-10-01T08:00:00Z`): SQLite's `datetime('now')`
/// wrote `2026-10-01 08:00:00`, UTC as well; a bare day is taken at noon
fn moment(at: &str) -> String {
    let at = at.trim();
    match at.len() {
        10 => format!("{at}T12:00:00Z"),
        _ if at.ends_with('Z') => at.replacen(' ', "T", 1),
        _ => format!("{}Z", at.replacen(' ', "T", 1)),
    }
}

/// A document as the history migration reads it
struct Backfill {
    id: i64,
    number: Option<String>,
    exported: bool,
    draft: String,
    total: Option<String>,
    exported_at: Option<String>,
    updated_at: String,
    issued_on: Option<String>,
    archived_at: Option<String>,
    paid_at: Option<String>,
}

/// What a version is compared by: the document's own data, not the settings frozen with it
fn snapshot_of(draft: &InvoiceDraft) -> Result<String> {
    Ok(serde_json::to_string(&InvoiceDraft {
        frozen: None,
        frozen_layout: None,
        ..draft.clone()
    })?)
}

fn insert_event(
    conn: &Connection,
    invoice_id: i64,
    kind: EventKind,
    at: &str,
    on: Option<&str>,
    detail: &EventDetail,
    snapshot: Option<&InvoiceDraft>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO document_events (invoice_id, kind, at, on_date, detail, snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            invoice_id,
            kind.as_str(),
            at,
            on,
            serde_json::to_string(detail)?,
            snapshot.map(snapshot_of).transpose()?
        ],
    )?;
    Ok(())
}

/// Notes an event now
pub(crate) fn record(
    conn: &Connection,
    invoice_id: i64,
    kind: EventKind,
    on: Option<&str>,
    detail: &EventDetail,
    snapshot: Option<&InvoiceDraft>,
) -> Result<()> {
    insert_event(conn, invoice_id, kind, &now(), on, detail, snapshot)
}

/// Everything of a document's history goes with a deleted draft
pub(crate) fn forget(conn: &Connection, invoice_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM document_events WHERE invoice_id = ?1",
        [invoice_id],
    )?;
    conn.execute("DELETE FROM payments WHERE invoice_id = ?1", [invoice_id])?;
    Ok(())
}

/// The payments of one document, oldest first
pub(crate) fn payments_of(conn: &Connection, invoice_id: i64) -> Result<Vec<Payment>> {
    let mut statement = conn.prepare(
        "SELECT id, amount, paid_on FROM payments WHERE invoice_id = ?1 ORDER BY paid_on, id",
    )?;
    let payments = statement
        .query_map([invoice_id], |r| {
            Ok(Payment {
                id: r.get(0)?,
                amount: r.get(1)?,
                paid_on: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(payments)
}

/// The payments of all documents, by document, for a list
pub(crate) fn all_payments(conn: &Connection) -> Result<HashMap<i64, Vec<Payment>>> {
    let mut statement =
        conn.prepare("SELECT invoice_id, id, amount, paid_on FROM payments ORDER BY paid_on, id")?;
    let rows = statement.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Payment {
                id: r.get(1)?,
                amount: r.get(2)?,
                paid_on: r.get(3)?,
            },
        ))
    })?;
    let mut by_invoice: HashMap<i64, Vec<Payment>> = HashMap::new();
    for row in rows {
        let (invoice, payment) = row?;
        by_invoice.entry(invoice).or_default().push(payment);
    }
    Ok(by_invoice)
}

/// A stored amount; what is no number counts as nothing
pub(crate) fn decimal(text: &str) -> Decimal {
    Decimal::from_str(text.trim()).unwrap_or_default()
}

/// Removes a payment and notes that it was taken back, returns its invoice
fn take_back(conn: &Connection, payment_id: i64) -> Result<i64> {
    let (invoice_id, amount, paid_on): (i64, String, String) = conn
        .query_row(
            "SELECT invoice_id, amount, paid_on FROM payments WHERE id = ?1",
            [payment_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("unknown payment"))?;
    conn.execute("DELETE FROM payments WHERE id = ?1", [payment_id])?;
    let detail = EventDetail {
        amount: Some(amount),
        ..EventDetail::default()
    };
    record(
        conn,
        invoice_id,
        EventKind::PaymentRemoved,
        Some(&paid_on),
        &detail,
        None,
    )?;
    Ok(invoice_id)
}

/// What is still open on an invoice: its total minus what was paid, never below zero
fn open_of(invoice: &StoredInvoice) -> Decimal {
    let (paid, _) = settle(invoice.total.as_deref(), &invoice.payments);
    (decimal(invoice.total.as_deref().unwrap_or("0"))
        - paid.as_deref().map_or(Decimal::ZERO, decimal))
    .max(Decimal::ZERO)
}

/// What the payments add up to (none without payments) and the day the document was paid
/// completely on: the day of the payment that reached its total
pub(crate) fn settle(
    total: Option<&str>,
    payments: &[Payment],
) -> (Option<String>, Option<String>) {
    if payments.is_empty() {
        return (None, None);
    }
    let target = total.map(decimal);
    let mut sum = Decimal::ZERO;
    let mut paid_at = None;
    for payment in payments {
        sum += decimal(&payment.amount);
        if paid_at.is_none() && target.is_none_or(|target| sum >= target) {
            paid_at = Some(payment.paid_on.clone());
        }
    }
    // a later payment that took the sum below the total again is no case: payments only add
    (Some(sum.normalize().to_string()), paid_at)
}

impl Store {
    /// A document's history as it was recorded; the day an event is about (a payment received
    /// earlier, a backdated invoice) is shown with it, it does not reorder the history
    pub fn events(&self, invoice_id: i64) -> Result<Vec<Event>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, invoice_id, kind, at, on_date, detail, snapshot IS NOT NULL
             FROM document_events WHERE invoice_id = ?1 ORDER BY at, id",
        )?;
        let rows = statement.query_map([invoice_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, bool>(6)?,
            ))
        })?;
        rows.filter_map(|row| match row {
            Ok((id, invoice_id, kind, at, on, detail, has_snapshot)) => {
                // a kind written by a newer version is left out
                let kind = EventKind::from_column(&kind)?;
                Some(serde_json::from_str(&detail).map_err(Into::into).map(
                    |detail: EventDetail| Event {
                        id,
                        invoice_id,
                        kind,
                        at,
                        on,
                        detail,
                        has_snapshot,
                    },
                ))
            }
            Err(e) => Some(Err(e.into())),
        })
        .collect()
    }

    /// The data a version was exported or baked with
    pub fn snapshot(&self, event_id: i64) -> Result<InvoiceDraft> {
        let json: Option<String> = self
            .conn()
            .query_row(
                "SELECT snapshot FROM document_events WHERE id = ?1",
                [event_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let json = json.ok_or_else(|| anyhow!("the event keeps no snapshot"))?;
        Ok(serde_json::from_str(&json)?)
    }

    /// How many times the document was exported as a draft
    pub fn draft_exports(&self, invoice_id: i64) -> Result<u32> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM document_events WHERE invoice_id = ?1 AND kind = ?2",
            params![invoice_id, EventKind::DraftExported.as_str()],
            |r| r.get(0),
        )?)
    }

    /// Notes that a draft was exported as it is in `draft` (what was on screen), returns its version
    pub fn record_draft_export(&self, invoice_id: i64, draft: &InvoiceDraft) -> Result<u32> {
        let stored = self.invoice(invoice_id)?;
        if stored.status != Status::Draft {
            bail!("only a draft is exported as a version");
        }
        let version = self.draft_exports(invoice_id)? + 1;
        let detail = EventDetail {
            version: Some(version),
            ..EventDetail::default()
        };
        let conn = self.conn();
        record(
            &conn,
            invoice_id,
            EventKind::DraftExported,
            None,
            &detail,
            Some(draft),
        )?;
        Ok(version)
    }

    /// Notes a payment received on a created invoice. The amount is gross and above zero, written
    /// either way (`1.130,50`, `1130.50`).
    pub fn add_payment(
        &self,
        invoice_id: i64,
        amount: &str,
        paid_on: &str,
    ) -> Result<StoredInvoice> {
        en16931::Date::parse(paid_on)
            .with_context(|| format!("the date of payment {paid_on:?} is not a date"))?;
        let value = crate::draft::decimal(amount)
            .with_context(|| format!("the amount {amount:?} is not a number"))?;
        if value <= Decimal::ZERO {
            bail!("a payment is above zero");
        }
        let stored = self.invoice(invoice_id)?;
        if stored.kind != Kind::Invoice || stored.status != Status::Exported {
            bail!("only an exported invoice can be paid");
        }
        // what is open is the most a payment can be, the invoice is never paid twice
        if value > open_of(&stored) {
            bail!("Die Zahlung ist höher als der offene Betrag.");
        }
        let amount = value.normalize().to_string();
        {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO payments (invoice_id, amount, paid_on, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![invoice_id, amount, paid_on, now()],
            )?;
            let detail = EventDetail {
                amount: Some(amount),
                ..EventDetail::default()
            };
            record(
                &tx,
                invoice_id,
                EventKind::Payment,
                Some(paid_on),
                &detail,
                None,
            )?;
            tx.commit()?;
        }
        self.invoice(invoice_id)
    }

    /// Takes a payment back, as an event of its own
    pub fn remove_payment(&self, payment_id: i64) -> Result<StoredInvoice> {
        let invoice_id = {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            let invoice_id = take_back(&tx, payment_id)?;
            tx.commit()?;
            invoice_id
        };
        self.invoice(invoice_id)
    }

    /// Marks an exported invoice as paid completely on a date: a payment of what is still open. `None`
    /// takes all its payments back, it is open again. Offers are not paid.
    pub fn set_paid(&self, invoice_id: i64, paid_on: Option<&str>) -> Result<StoredInvoice> {
        let stored = self.invoice(invoice_id)?;
        if stored.kind != Kind::Invoice || stored.status != Status::Exported {
            bail!("only an exported invoice can be paid");
        }
        let Some(paid_on) = paid_on else {
            // all or none: open again in one step
            {
                let conn = self.conn();
                let tx = conn.unchecked_transaction()?;
                for payment in &stored.payments {
                    take_back(&tx, payment.id)?;
                }
                tx.commit()?;
            }
            return self.invoice(invoice_id);
        };
        let open = open_of(&stored);
        if open <= Decimal::ZERO {
            en16931::Date::parse(paid_on)
                .with_context(|| format!("the date of payment {paid_on:?} is not a date"))?;
            return Ok(stored);
        }
        self.add_payment(invoice_id, &open.to_string(), paid_on)
    }

    /// Marks what the customer said to a created offer on a day, or takes it back (`None`)
    pub fn set_answer(
        &self,
        invoice_id: i64,
        answer: Option<(Answer, &str)>,
    ) -> Result<StoredInvoice> {
        let stored = self.invoice(invoice_id)?;
        if stored.kind != Kind::Offer || stored.status != Status::Exported {
            bail!("only a created offer is answered");
        }
        if let Some((_, on)) = answer {
            en16931::Date::parse(on).with_context(|| format!("{on:?} is not a date"))?;
        }
        if let Some((Answer::Accepted, _)) = answer {
            self.may_accept(invoice_id)?;
        }
        {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "UPDATE invoices SET answer = ?1, answered_on = ?2 WHERE id = ?3",
                params![
                    answer.map(|(a, _)| a.as_str()),
                    answer.map(|(_, on)| on),
                    invoice_id
                ],
            )?;
            let (kind, on) = match answer {
                Some((Answer::Accepted, on)) => (EventKind::Accepted, Some(on)),
                Some((Answer::Declined, on)) => (EventKind::Declined, Some(on)),
                None if stored.answer.is_some() => (EventKind::AnswerRemoved, None),
                None => return Ok(stored),
            };
            record(&tx, invoice_id, kind, on, &EventDetail::default(), None)?;
            tx.commit()?;
        }
        self.invoice(invoice_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::EventKind::{
        Accepted, AnswerRemoved, Archived, Baked, Created, Declined, DraftExported,
        Payment as Paid, PaymentRemoved, Restored,
    };
    use crate::model::{InvoiceFiles, StoredFile};
    use crate::store::Sequence;

    fn kinds(store: &Store, id: i64) -> Vec<EventKind> {
        store
            .events(id)
            .unwrap()
            .into_iter()
            .map(|e| e.kind)
            .collect()
    }

    /// Creates a stored draft for good under a number
    fn bake(store: &Store, id: i64, number: &str) {
        let file = |path: &str| StoredFile {
            path: path.into(),
            sha256: "0".repeat(64),
        };
        let sequence = Sequence {
            next_number: number.into(),
            counter: 1,
            highest: None,
            floor: None,
        };
        let files = InvoiceFiles {
            pdf: file("x.pdf"),
            xml: None,
            regenerated: false,
        };
        store
            .finalize(id, &sequence, &settings(), None, "1130.50", &files)
            .unwrap();
    }

    #[test]
    fn a_document_tells_what_happened_to_it() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        // saving again is no event
        store
            .save_draft(Some(id), 1, Kind::Invoice, &draft(), None)
            .unwrap();
        let mut shown = draft();
        shown.note = Some("Erste Fassung".into());
        assert_eq!(store.record_draft_export(id, &shown).unwrap(), 1);
        assert_eq!(store.record_draft_export(id, &draft()).unwrap(), 2);
        bake(&store, id, "RE-2026-001");
        assert!(store.record_draft_export(id, &draft()).is_err());
        store.archive_invoice(id).unwrap();
        store.unarchive_invoice(id).unwrap();

        assert_eq!(
            kinds(&store, id),
            [
                Created,
                DraftExported,
                DraftExported,
                Baked,
                Archived,
                Restored
            ]
        );
        let events = store.events(id).unwrap();
        assert_eq!(events[2].detail.version, Some(2));
        assert_eq!(events[3].detail.number.as_deref(), Some("RE-2026-001"));
        assert_eq!(events[3].on.as_deref(), Some(draft().issue_date.as_str()));
        // the versions keep what was created and exported, without the frozen settings
        assert!(events[..4].iter().all(|e| e.has_snapshot) && !events[4].has_snapshot);
        assert_eq!(
            store.snapshot(events[1].id).unwrap().note.as_deref(),
            Some("Erste Fassung")
        );
        assert_eq!(store.snapshot(events[3].id).unwrap().frozen, None);
        assert!(store.snapshot(events[0].id).is_ok());
        assert!(store.snapshot(events[4].id).is_err());
        assert_eq!(store.draft_exports(id).unwrap(), 2);
    }

    #[test]
    fn a_copy_and_an_invoice_from_an_offer_know_where_they_come_from() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap()
            .id;
        let copy = store.duplicate(offer, "2026-12-01").unwrap().id;
        bake(&store, offer, "AN-2026-001");
        let invoice = store
            .invoice_from_offer(offer, "2026-12-01", &settings())
            .unwrap()
            .id;

        assert_eq!(store.events(copy).unwrap()[0].detail.from, Some(offer));
        assert_eq!(store.events(invoice).unwrap()[0].detail.from, Some(offer));
        assert_eq!(store.events(offer).unwrap()[0].detail.from, None);
        // a deleted draft takes its history with it
        store.delete_draft(copy).unwrap();
        assert!(store.events(copy).unwrap().is_empty());
    }

    #[test]
    fn payments_add_up_and_are_taken_back_as_events() {
        let store = Store::in_memory().unwrap();
        let id = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        assert!(store.add_payment(id, "100", "2026-10-20").is_err());
        bake(&store, id, "RE-2026-001");
        assert!(store.add_payment(id, "0", "2026-10-20").is_err());
        assert!(store.add_payment(id, "100", "20.10.2026").is_err());
        assert!(store.add_payment(id, "viel", "2026-10-20").is_err());

        // never more than is open
        assert!(store.add_payment(id, "1130.51", "2026-10-20").is_err());
        let partly = store.add_payment(id, "1.000,00", "2026-10-20").unwrap();
        assert_eq!(partly.payments.len(), 1);
        assert_eq!(partly.payments[0].amount, "1000");
        assert_eq!(partly.paid_at, None);
        let listed = store.invoices(None, None, false).unwrap();
        assert_eq!(listed[0].paid.as_deref(), Some("1000"));
        assert_eq!(listed[0].paid_at, None);

        assert!(store.add_payment(id, "130.51", "2026-10-25").is_err());
        // "paid" pays what is open
        let paid = store.set_paid(id, Some("2026-10-25")).unwrap();
        assert_eq!(paid.payments[1].amount, "130.5");
        assert_eq!(paid.paid_at.as_deref(), Some("2026-10-25"));

        let open = store.remove_payment(paid.payments[1].id).unwrap();
        assert_eq!(open.paid_at, None);
        assert_eq!(store.set_paid(id, None).unwrap().payments, []);

        assert_eq!(
            kinds(&store, id),
            [Created, Baked, Paid, Paid, PaymentRemoved, PaymentRemoved]
        );
    }

    #[test]
    fn an_offer_is_accepted_or_declined_and_that_can_be_taken_back() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap()
            .id;
        let accepted = Some((Answer::Accepted, "2026-10-22"));
        assert!(store.set_answer(offer, accepted).is_err());
        bake(&store, offer, "AN-2026-001");
        assert!(store
            .set_answer(offer, Some((Answer::Accepted, "22.10.")))
            .is_err());

        let answered = store.set_answer(offer, accepted).unwrap();
        assert_eq!(answered.answer, Some(Answer::Accepted));
        assert_eq!(answered.answered_on.as_deref(), Some("2026-10-22"));
        assert_eq!(
            store.invoices(None, None, false).unwrap()[0].answer,
            Some(Answer::Accepted)
        );
        store
            .set_answer(offer, Some((Answer::Declined, "2026-10-23")))
            .unwrap();
        assert_eq!(store.set_answer(offer, None).unwrap().answer, None);
        // taking back nothing is nothing
        store.set_answer(offer, None).unwrap();

        assert_eq!(
            kinds(&store, offer),
            [Created, Baked, Accepted, Declined, AnswerRemoved]
        );
        // invoices are paid, not answered
        let invoice = store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        bake(&store, invoice, "RE-2026-001");
        assert!(store.set_answer(invoice, accepted).is_err());
    }

    #[test]
    fn a_version_eleven_database_gets_its_history_from_what_it_stored() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::migrate(&conn).unwrap();
        conn.execute_batch(
            "DROP TABLE document_events; DROP TABLE payments;
             ALTER TABLE invoices DROP COLUMN answer; ALTER TABLE invoices DROP COLUMN answered_on;
             ALTER TABLE invoices ADD COLUMN paid_at TEXT;
             PRAGMA user_version = 11;",
        )
        .unwrap();
        let json = serde_json::to_string(&draft()).unwrap();
        conn.execute(
            "INSERT INTO invoices (id, context_id, kind, number, status, draft, total, exported_at,
                 paid_at, updated_at, archived_at, issued_on)
             VALUES (1, 1, 'invoice', 'RE-2026-001', 'exported', ?1, '1130.50',
                 '2026-10-01 08:00:00', '2026-10-05', '2026-10-01 08:00:00',
                 '2026-10-09T08:00:00Z', '2026-10-01'),
                (2, 1, 'offer', NULL, 'draft', ?1, NULL, NULL, NULL, '2026-10-02T08:00:00Z', NULL,
                 NULL)",
            [&json],
        )
        .unwrap();

        let store = Store::from_connection(conn).unwrap();

        assert_eq!(kinds(&store, 1), [Baked, Paid, Archived]);
        // a draft of before has no known start
        assert!(kinds(&store, 2).is_empty());
        let invoice = store.invoice(1).unwrap();
        assert_eq!(invoice.paid_at.as_deref(), Some("2026-10-05"));
        assert_eq!(invoice.payments[0].amount, "1130.50");
        assert!(!crate::store::has_column(&store.conn(), "invoices", "paid_at").unwrap());
        assert!(store.events(1).unwrap()[0].has_snapshot);
        // SQLite's own times are written the way events keep them
        assert_eq!(store.events(1).unwrap()[0].at, "2026-10-01T08:00:00Z");
    }

    fn payment(amount: &str, paid_on: &str) -> Payment {
        Payment {
            id: 0,
            amount: amount.into(),
            paid_on: paid_on.into(),
        }
    }

    #[test]
    fn stored_times_are_written_the_way_events_keep_them() {
        assert_eq!(moment("2026-10-01 08:00:00"), "2026-10-01T08:00:00Z");
        assert_eq!(moment("2026-10-01T08:00:00Z"), "2026-10-01T08:00:00Z");
        assert_eq!(moment("2026-10-01"), "2026-10-01T12:00:00Z");
    }

    #[test]
    fn a_document_is_paid_on_the_day_its_payments_reach_the_total() {
        let payments = [
            payment("300", "2026-10-01"),
            payment("700.50", "2026-10-10"),
            payment("130", "2026-10-20"),
        ];

        assert_eq!(
            settle(Some("1130.50"), &payments[..2]),
            (Some("1000.5".into()), None)
        );
        assert_eq!(
            settle(Some("1130.50"), &payments),
            (Some("1130.5".into()), Some("2026-10-20".into()))
        );
        assert_eq!(settle(Some("1130.50"), &[]), (None, None));
        // more than the total is paid too
        assert_eq!(
            settle(Some("100"), &[payment("150", "2026-10-01")])
                .1
                .as_deref(),
            Some("2026-10-01")
        );
    }
}
