//! Vorgänge (`cases`): every offer and invoice is in exactly one, a plain new document makes its
//! own. A Vorgang belongs to one context and one customer and has a number of its own, given when
//! it is made, shown only where the user puts it (`{vorgang_number}`).
//!
//! The customer is the Vorgang's: while it holds a single draft it made itself, changing the draft's
//! customer changes it; once it has a second document or a created one, or when it was started for
//! its customer (`Neuer Vorgang`), the customer is locked. Locked is read from the data, never
//! stored. So is where a Vorgang stands (`status`).

use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, bail, Result};
use en16931::Date;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

use crate::model::{Answer, Case, CaseStage, Event, InvoiceDraft, Kind, Payment, Settings, Status};
use crate::numbering::{Given, Pattern, DEFAULT_CASE_PATTERN};
use crate::relations::{lapsed, reference, Relation, RELATION_COLUMNS};
use crate::store::{context_settings, now, Sequence, Store};

/// The Vorgänge table and every document's Vorgang. What older versions stored gets one Vorgang
/// per group of linked documents (split where their customers differ), numbered per context in
/// the order of their first issue date.
pub(crate) fn migrate_to_cases(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS cases (
             id INTEGER PRIMARY KEY,
             context_id INTEGER NOT NULL,
             number TEXT NOT NULL,
             counter INTEGER NOT NULL,
             issued_on TEXT NOT NULL,
             customer_id INTEGER,
             title TEXT NOT NULL,
             created_at TEXT NOT NULL,
             started INTEGER NOT NULL DEFAULT 0,
             UNIQUE (context_id, number)
         );",
    )?;
    if !crate::store::has_column(&tx, "invoices", "case_id")? {
        tx.execute_batch(
            "ALTER TABLE invoices ADD COLUMN case_id INTEGER;
             CREATE INDEX IF NOT EXISTS invoices_case ON invoices (case_id);",
        )?;
    }
    let documents = {
        let mut statement = tx.prepare(
            "SELECT id, context_id, draft, kind, number FROM invoices WHERE case_id IS NULL
             ORDER BY id",
        )?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                Kind::from_column(&r.get::<_, String>(3)?),
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    // the links went with schema 15, a database stamped back from there has none
    let has_links: bool = tx.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'document_links'",
        [],
        |r| r.get(0),
    )?;
    let links = if has_links {
        let mut statement = tx.prepare("SELECT offer_id, invoice_id FROM document_links")?;
        let rows = statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    // linked documents end up with the same root
    let mut parent: HashMap<i64, i64> = documents.iter().map(|(id, ..)| (*id, *id)).collect();
    for (offer, invoice) in links {
        if parent.contains_key(&offer) && parent.contains_key(&invoice) {
            let (a, b) = (root(&mut parent, offer), root(&mut parent, invoice));
            if a != b {
                parent.insert(a.max(b), a.min(b));
            }
        }
    }
    // a group: its context, customer and documents with their issue dates, kinds and numbers
    let mut groups: BTreeMap<GroupKey, Vec<Grouped>> = BTreeMap::new();
    for (id, context, json, kind, number) in &documents {
        let draft: InvoiceDraft = serde_json::from_str(json)?;
        let key = (*context, root(&mut parent, *id), draft.customer.id);
        groups.entry(key).or_default().push(Grouped {
            id: *id,
            day: draft.issue_date.clone(),
            kind: *kind,
            number: number.clone(),
        });
    }
    let first = |docs: &[Grouped]| {
        docs.iter()
            .min_by(|a, b| (&a.day, a.id).cmp(&(&b.day, b.id)))
            .cloned()
    };
    let mut ordered: Vec<_> = groups.into_iter().collect();
    ordered.sort_by(|((ca, _, _), a), ((cb, _, _), b)| {
        let day = |docs: &[Grouped]| first(docs).map(|d| d.day);
        (ca, day(a), a[0].id).cmp(&(cb, day(b), b[0].id))
    });
    for ((context, _, customer), docs) in ordered {
        let settings = numbering_settings(&tx, context);
        let issued = docs
            .iter()
            .filter_map(|d| Date::parse(&d.day).ok())
            .min()
            .unwrap_or_else(|| Date::parse(&crate::utils::today()).expect("today is a date"));
        // older versions named no Vorgang: it is called after its first document
        let title = first(&docs)
            .map(|d| title_after(d.kind, d.number.as_deref(), &d.day))
            .unwrap_or_default();
        let case = insert_case(&tx, context, &settings, customer, &title, issued)?;
        for Grouped { id, .. } in docs {
            tx.execute(
                "UPDATE invoices SET case_id = ?1 WHERE id = ?2",
                params![case, id],
            )?;
        }
    }
    tx.execute_batch("PRAGMA user_version = 13;")?;
    tx.commit()?;
    Ok(())
}

/// Whether a Vorgang was started on its own (`Neuer Vorgang`) rather than made by its first document.
/// Came with schema 13 before that was released, so databases at 13 may lack it: added on every open.
pub(crate) fn ensure_started(conn: &Connection) -> Result<()> {
    if !crate::store::has_column(conn, "cases", "started")? {
        conn.execute_batch("ALTER TABLE cases ADD COLUMN started INTEGER NOT NULL DEFAULT 0;")?;
    }
    Ok(())
}

/// Context, root of the linked documents and customer: documents of one key share a Vorgang
type GroupKey = (i64, i64, Option<i64>);

/// A document of an older version on its way into a Vorgang
#[derive(Clone)]
struct Grouped {
    id: i64,
    day: String,
    kind: Kind,
    number: Option<String>,
}

/// What a Vorgang is called that was made without a name: after a document of it, by its kind and
/// number, or by its day while it is a draft (`Angebot AN-2026-001`, `Rechnung vom 03.11.2026`)
pub(crate) fn title_after(kind: Kind, number: Option<&str>, issue_date: &str) -> String {
    let noun = match kind {
        Kind::Offer => "Angebot",
        Kind::Invoice => "Rechnung",
    };
    if let Some(number) = number {
        return format!("{noun} {number}");
    }
    // the day as the app writes it, an unreadable one as it is
    let day = Date::parse(issue_date).map_or_else(
        |_| issue_date.to_owned(),
        |d| {
            let iso = d.to_string();
            let parts: Vec<&str> = iso.split('-').collect();
            match parts.as_slice() {
                [y, m, d] => format!("{d}.{m}.{y}"),
                _ => iso,
            }
        },
    );
    format!("{noun} vom {day}")
}

/// A name for a Vorgang as typed: trimmed, never blank
pub(crate) fn named(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Ein Vorgang braucht eine Bezeichnung.");
    }
    Ok(title)
}

/// The root of a document among the linked ones, shortening the way there for the next look
fn root(parent: &mut HashMap<i64, i64>, id: i64) -> i64 {
    let up = parent.get(&id).copied().unwrap_or(id);
    if up == id {
        return id;
    }
    let top = root(parent, up);
    parent.insert(id, top);
    top
}

/// The numbers of a context's Vorgänge with their counters and days, for the next one
fn given(conn: &Connection, context_id: i64) -> Result<Vec<Given>> {
    let mut statement =
        conn.prepare("SELECT number, counter, issued_on FROM cases WHERE context_id = ?1")?;
    let rows = statement.query_map([context_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, u32>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    Ok(rows
        .filter_map(std::result::Result::ok)
        .filter_map(|(number, counter, day)| {
            Some(Given {
                number,
                counter,
                date: Date::parse(&day).ok()?,
            })
        })
        .collect())
}

/// The pattern the settings give, the default when theirs does not parse
fn pattern(settings: &Settings) -> Pattern {
    Pattern::parse(&settings.cases.number_pattern)
        .or_else(|_| Pattern::parse(DEFAULT_CASE_PATTERN))
        .expect("the default pattern parses")
}

/// What the counters of Vorgänge are called in `number_floors`, beside the kinds of documents
const FLOOR_KIND: &str = "case";

fn floor(conn: &Connection, context_id: i64, period: &str) -> Result<Option<u32>> {
    Ok(conn
        .query_row(
            "SELECT next FROM number_floors WHERE context_id = ?1 AND kind = ?2 AND period = ?3",
            params![context_id, FLOOR_KIND, period],
            |r| r.get(0),
        )
        .optional()?)
}

/// Where the numbers of a context's Vorgänge stand in the period of `date`: the number the next
/// one gets, with its counter, the highest given and the floor set for the period. A number that
/// is taken already (written by hand into another pattern) moves the counter on.
pub(crate) fn sequence(
    conn: &Connection,
    context_id: i64,
    settings: &Settings,
    date: Date,
) -> Result<Sequence> {
    let pattern = pattern(settings);
    let calendar = settings.calendar();
    let reset = settings.cases.number_reset;
    let given = given(conn, context_id)?;
    let period = calendar.period(reset, date);
    let highest = calendar.highest(reset, &period, &given);
    let floor = floor(conn, context_id, period.key())?;
    let mut counter = highest.map_or(1, |g| g.counter + 1).max(floor.unwrap_or(0));
    loop {
        let number = pattern.number(date, calendar, counter);
        if given.iter().all(|g| g.number != number) {
            return Ok(Sequence {
                next_number: number,
                counter,
                highest: highest.map(|g| g.number.clone()),
                // a floor the numbers have passed decides nothing any more
                floor: floor.filter(|floor| *floor > highest.map_or(0, |g| g.counter)),
            });
        }
        counter += 1;
    }
}

fn next_number(
    conn: &Connection,
    context_id: i64,
    settings: &Settings,
    date: Date,
) -> Result<(String, u32)> {
    let sequence = sequence(conn, context_id, settings, date)?;
    Ok((sequence.next_number, sequence.counter))
}

fn insert_case(
    conn: &Connection,
    context_id: i64,
    settings: &Settings,
    customer_id: Option<i64>,
    title: &str,
    date: Date,
) -> Result<i64> {
    let title = named(title)?;
    let (number, counter) = next_number(conn, context_id, settings, date)?;
    conn.execute(
        "INSERT INTO cases (context_id, number, counter, issued_on, customer_id, title, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            context_id,
            number,
            counter,
            date.to_string(),
            customer_id,
            title,
            now()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The settings a context numbers its Vorgänge by; a context without any stored numbers by the
/// default, numbering a Vorgang is no reason to refuse a document
fn numbering_settings(conn: &Connection, context_id: i64) -> Settings {
    context_settings(conn, context_id).unwrap_or_default()
}

/// A new Vorgang of a context for a customer, made today
pub(crate) fn new_case(
    conn: &Connection,
    context_id: i64,
    customer_id: Option<i64>,
    title: &str,
) -> Result<i64> {
    let settings = numbering_settings(conn, context_id);
    let today = Date::parse(&crate::utils::today())?;
    insert_case(conn, context_id, &settings, customer_id, title, today)
}

/// The Vorgang a document is in
pub(crate) fn case_of(conn: &Connection, invoice_id: i64) -> Result<i64> {
    conn.query_row(
        "SELECT case_id FROM invoices WHERE id = ?1",
        [invoice_id],
        |r| r.get::<_, Option<i64>>(0),
    )
    .optional()?
    .flatten()
    .ok_or_else(|| anyhow!("the document is in no Vorgang"))
}

/// Whether a Vorgang's customer is locked: it was started for it, or has a second document or a
/// created one
fn locked(conn: &Connection, case_id: i64) -> Result<bool> {
    let (started, documents, created): (bool, u32, u32) = conn.query_row(
        "SELECT started,
             (SELECT COUNT(*) FROM invoices WHERE case_id = cases.id),
             (SELECT COUNT(*) FROM invoices WHERE case_id = cases.id AND status = 'exported')
         FROM cases WHERE id = ?1",
        [case_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(started || documents > 1 || created > 0)
}

/// Keeps a draft's customer and its Vorgang's together before the draft is saved: a draft alone in
/// its Vorgang takes the Vorgang's customer with it, in a locked one it must be the Vorgang's
pub(crate) fn follow_customer(
    conn: &Connection,
    case_id: i64,
    customer_id: Option<i64>,
) -> Result<()> {
    let current: Option<i64> = conn
        .query_row(
            "SELECT customer_id FROM cases WHERE id = ?1",
            [case_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("unknown Vorgang"))?;
    if current == customer_id {
        return Ok(());
    }
    let locked = locked(conn, case_id)?;
    // a draft that has no customer yet changes nothing, the editor gives it the Vorgang's
    if locked && customer_id.is_none() {
        return Ok(());
    }
    if current.is_some() && locked {
        bail!("Der Kunde des Vorgangs ist festgelegt, alle seine Dokumente gehen an ihn.");
    }
    conn.execute(
        "UPDATE cases SET customer_id = ?1 WHERE id = ?2",
        params![customer_id, case_id],
    )?;
    Ok(())
}

/// A Vorgang left without documents goes with its last one
/// Whether it went
pub(crate) fn drop_if_empty(conn: &Connection, case_id: i64) -> Result<bool> {
    let changed = conn.execute(
        "DELETE FROM cases WHERE id = ?1
             AND NOT EXISTS (SELECT 1 FROM invoices WHERE case_id = ?1)",
        [case_id],
    )?;
    if changed > 0 {
        crate::case_links::forget(conn, Some(case_id))?;
    }
    Ok(changed > 0)
}

/// One document as `status` reads it
#[derive(Clone)]
pub(crate) struct Standing {
    pub kind: Kind,
    pub status: Status,
    pub total: Option<String>,
    pub paid: Decimal,
    pub answer: Option<Answer>,
    pub relation: Option<Relation>,
    /// Out of the running: another offer of its group accepted, or replaced (see `relations`)
    pub lapsed: bool,
    pub superseded: bool,
    /// An invoice taken back by a created Stornorechnung, or that Stornorechnung: it bills nothing
    pub settled: bool,
    /// A created invoice: the day it is due, by the payment days it was made with
    pub due: Option<String>,
}

/// The day an invoice issued on `issue_date` is due, `days` after (none: the same day)
fn due_on(issue_date: Option<&str>, days: Option<u32>) -> Option<String> {
    let issued = chrono::NaiveDate::parse_from_str(issue_date?, "%Y-%m-%d").ok()?;
    issued
        .checked_add_days(chrono::Days::new(u64::from(days.unwrap_or(0))))
        .map(|d| d.format("%Y-%m-%d").to_string())
}

fn decimal(text: Option<&str>) -> Decimal {
    text.map(crate::history::decimal).unwrap_or_default()
}

/// Where a Vorgang stands and what its sums are, read from its documents and, for a collected one,
/// its part of the invoices collecting it (`collected_parts`), on `today`. The one place that
/// decides it.
pub(crate) fn status(documents: &[Standing], today: &str) -> (CaseStage, [Decimal; 5]) {
    let created = |kind: Kind| {
        documents
            .iter()
            .filter(move |d| d.kind == kind && d.status == Status::Exported)
    };
    // what is still on offer: neither lapsed nor replaced; an alternative only once it is taken,
    // until then its group is offered once, by the offer it is a variant of
    let running = || created(Kind::Offer).filter(|d| !d.lapsed && !d.superseded);
    let offered: Decimal = running()
        .filter(|d| d.answer != Some(Answer::Declined))
        .filter(|d| d.relation != Some(Relation::Alternative) || d.answer.is_some())
        .map(|d| decimal(d.total.as_deref()))
        .sum();
    let accepted: Decimal = created(Kind::Offer)
        .filter(|d| d.answer == Some(Answer::Accepted))
        .map(|d| decimal(d.total.as_deref()))
        .sum();
    let invoiced: Decimal = created(Kind::Invoice)
        .map(|d| decimal(d.total.as_deref()))
        .sum();
    let paid: Decimal = created(Kind::Invoice).map(|d| d.paid).sum();
    let open = (invoiced - paid).max(Decimal::ZERO);
    // the furthest step: what is billed, else what is offered, else what is being written
    let billing = || created(Kind::Invoice).filter(|d| !d.settled);
    // an invoice not paid in full past its day
    let overdue = billing().any(|d| {
        d.paid < decimal(d.total.as_deref()) && d.due.as_deref().is_some_and(|due| due < today)
    });
    // accepted offers not billed in full yet: an Abschlag or a Teilrechnung so far
    let left_to_bill = (accepted - invoiced).round_dp(2) > Decimal::ZERO;
    let drafted = |kind: Kind| {
        documents
            .iter()
            .any(|d| d.kind == kind && d.status == Status::Draft)
    };
    // while the customer is to pay, the payment tells; once they did, what is left to bill
    let stage = if billing().next().is_some() {
        if overdue {
            CaseStage::Overdue
        } else if !open.is_zero() && paid.is_zero() {
            CaseStage::Invoiced
        } else if !open.is_zero() {
            CaseStage::PartlyPaid
        } else if left_to_bill {
            CaseStage::PartlyInvoiced
        } else {
            CaseStage::Paid
        }
    } else if created(Kind::Offer).any(|d| d.answer == Some(Answer::Accepted)) {
        CaseStage::Accepted
    } else if created(Kind::Offer).next().is_some() {
        if running().all(|d| d.answer == Some(Answer::Declined)) {
            CaseStage::Declined
        } else {
            CaseStage::Offered
        }
    } else if created(Kind::Invoice).next().is_some() {
        CaseStage::Cancelled
    } else if drafted(Kind::Invoice) {
        CaseStage::DraftingInvoice
    } else if drafted(Kind::Offer) {
        CaseStage::DraftingOffer
    } else {
        CaseStage::New
    };
    (stage, [offered, accepted, invoiced, paid, open])
}

fn money(value: Decimal) -> String {
    value.round_dp(2).normalize().to_string()
}

impl Store {
    /// The Vorgänge of a context (all with `None`), newest first; archived ones (every document
    /// archived) only when asked for
    pub fn cases(&self, context: Option<i64>, with_archived: bool) -> Result<Vec<Case>> {
        let conn = self.conn();
        let ids: Vec<i64> = {
            let mut statement = conn.prepare(
                "SELECT id FROM cases WHERE ?1 IS NULL OR context_id = ?1
                 ORDER BY created_at DESC, id DESC",
            )?;
            let ids = statement
                .query_map([context], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        };
        // the payments are read once for the whole list
        let payments = crate::history::all_payments(&conn)?;
        let cases = ids
            .into_iter()
            .map(|id| read_case(&conn, id, &payments))
            .collect::<Result<Vec<_>>>()?;
        Ok(cases
            .into_iter()
            .filter(|c| with_archived || !c.archived)
            .collect())
    }

    pub fn case(&self, id: i64) -> Result<Case> {
        let conn = self.conn();
        let payments = crate::history::all_payments(&conn)?;
        read_case(&conn, id, &payments)
    }

    /// The context a Vorgang belongs to, without reading its documents
    pub fn case_context(&self, id: i64) -> Result<i64> {
        self.conn()
            .query_row("SELECT context_id FROM cases WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or_else(|| anyhow!("unknown Vorgang"))
    }
}

/// The documents of a Vorgang as `status` reads them, the newest first, with the customer's name
/// on the newest and whether all of them are archived (a Vorgang without documents is not)
fn documents_of(
    conn: &Connection,
    id: i64,
    payments: &HashMap<i64, Vec<Payment>>,
) -> Result<(Vec<Standing>, Option<String>, bool)> {
    let mut statement = conn.prepare(&format!(
        "SELECT id, kind, status, total, answer, draft, archived_at,
             json_extract(draft, '$.corrects.kind') AS correction,
             json_extract(draft, '$.issue_date') AS issued,
             json_extract(draft, '$.frozen.payment_days') AS days, {RELATION_COLUMNS}
         FROM invoices WHERE case_id = ?1 ORDER BY updated_at DESC, id DESC",
    ))?;
    let rows = statement
        .query_map([id], |r| {
            let kind = Kind::from_column(&r.get::<_, String>("kind")?);
            let status = if r.get::<_, String>("status")? == "exported" {
                Status::Exported
            } else {
                Status::Draft
            };
            let answer = r
                .get::<_, Option<String>>("answer")?
                .as_deref()
                .and_then(Answer::from_column);
            let paid = payments
                .get(&r.get::<_, i64>("id")?)
                .map(|p| p.iter().map(|p| decimal(Some(&p.amount))).sum())
                .unwrap_or_default();
            let standing = Standing {
                kind,
                status,
                total: r.get("total")?,
                paid,
                answer,
                relation: r
                    .get::<_, Option<String>>("relation")?
                    .as_deref()
                    .and_then(Relation::from_column),
                lapsed: lapsed(kind, status, answer, reference(r, "taken_by")?.is_some()),
                superseded: reference(r, "superseded_by")?.is_some(),
                settled: reference(r, "cancelled_by")?.is_some()
                    || r.get::<_, Option<String>>("correction")?.as_deref() == Some("cancel"),
                due: (kind == Kind::Invoice && status == Status::Exported)
                    .then(|| -> rusqlite::Result<Option<String>> {
                        Ok(due_on(
                            r.get::<_, Option<String>>("issued")?.as_deref(),
                            r.get("days")?,
                        ))
                    })
                    .transpose()?
                    .flatten(),
            };
            let archived = r.get::<_, Option<String>>("archived_at")?.is_some();
            Ok((standing, r.get::<_, String>("draft")?, archived))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let newest_customer = rows
        .first()
        .map(|(_, draft, _)| serde_json::from_str::<InvoiceDraft>(draft))
        .transpose()?
        .map(|draft| draft.customer.name);
    let archived = !rows.is_empty() && rows.iter().all(|(_, _, archived)| *archived);
    Ok((
        rows.into_iter().map(|(standing, _, _)| standing).collect(),
        newest_customer,
        archived,
    ))
}

/// The part of each created invoice of a Vorgang collecting `id` that bills the offers of `id`, as
/// an invoice of its own: total and payments in the share its positions have of the invoice's net.
/// A cancelled invoice counts for none (its Storno mirrors it without referring to offers).
fn collected_parts(
    conn: &Connection,
    id: i64,
    payments: &HashMap<i64, Vec<Payment>>,
) -> Result<Vec<Standing>> {
    let mut statement = conn.prepare(
        "SELECT i.id, i.total, i.draft FROM invoices i
             JOIN case_links l ON l.follower_id = i.case_id AND l.kind = 'collect'
         WHERE l.source_id = ?1 AND i.kind = 'invoice' AND i.status = 'exported'
             AND NOT EXISTS (SELECT 1 FROM invoices s WHERE s.status = 'exported'
                 AND json_extract(s.draft, '$.corrects.kind') = 'cancel'
                 AND json_extract(s.draft, '$.corrects.invoice_id') = i.id)",
    )?;
    let rows = statement
        .query_map([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let offers: Vec<i64> = conn
        .prepare("SELECT id FROM invoices WHERE case_id = ?1 AND kind = 'offer'")?
        .query_map([id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let net = |p: &crate::model::Position| {
        crate::billing::quantity(p) * crate::draft::decimal(&p.price).unwrap_or_default()
    };
    let parts = rows
        .into_iter()
        .map(|(invoice, total, draft)| -> Result<Option<Standing>> {
            let draft: InvoiceDraft = serde_json::from_str(&draft)?;
            let all: Decimal = draft.positions.iter().map(net).sum();
            let ours: Decimal = draft
                .positions
                .iter()
                .filter(|p| p.source.is_some_and(|s| offers.contains(&s.offer_id)))
                .map(net)
                .sum();
            if all.is_zero() || ours.is_zero() {
                return Ok(None);
            }
            let share = ours / all;
            let paid: Decimal = payments
                .get(&invoice)
                .map(|p| p.iter().map(|p| decimal(Some(&p.amount))).sum())
                .unwrap_or_default();
            Ok(Some(Standing {
                kind: Kind::Invoice,
                status: Status::Exported,
                total: Some((decimal(total.as_deref()) * share).round_dp(2).to_string()),
                paid: (paid * share).round_dp(2),
                answer: None,
                relation: None,
                lapsed: false,
                superseded: false,
                settled: false,
                due: due_on(
                    Some(&draft.issue_date),
                    draft.frozen.as_ref().and_then(|f| f.payment_days),
                ),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(parts.into_iter().flatten().collect())
}

/// A Vorgang with what its documents say, `payments` of all documents by document
fn read_case(conn: &Connection, id: i64, payments: &HashMap<i64, Vec<Payment>>) -> Result<Case> {
    let (context_id, number, customer_id, title, created_at) = conn
        .query_row(
            "SELECT context_id, number, customer_id, title, created_at FROM cases WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow!("unknown Vorgang"))?;
    let (standings, newest_customer, archived) = documents_of(conn, id, payments)?;
    let customer_name = match newest_customer {
        Some(name) => name,
        None => customer_id
            .map(|customer| {
                conn.query_row(
                    "SELECT json FROM customers WHERE id = ?1",
                    [customer],
                    |r| r.get::<_, String>(0),
                )
                .optional()
            })
            .transpose()?
            .flatten()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .and_then(|json| json["name"].as_str().map(str::to_owned))
            .unwrap_or_default(),
    };
    let documents = u32::try_from(standings.len()).unwrap_or(u32::MAX);
    let standings: Vec<Standing> = standings
        .into_iter()
        .chain(collected_parts(conn, id, payments)?)
        .collect();
    let (stage, [offered, accepted, invoiced, paid, open]) =
        status(&standings, &crate::utils::today());
    Ok(Case {
        id,
        context_id,
        number,
        customer_id,
        customer_name,
        title,
        created_at,
        locked: locked(conn, id)?,
        documents,
        stage,
        offered: money(offered),
        accepted: money(accepted),
        invoiced: money(invoiced),
        paid: money(paid),
        open: money(open),
        archived,
        sources: crate::case_links::refs(conn, id, true)?,
        followers: crate::case_links::refs(conn, id, false)?,
    })
}

impl Store {
    /// A Vorgang without documents yet, started for a customer (`Neuer Vorgang`): its customer is
    /// locked from the start, every document of it is for that customer
    pub fn create_case(
        &self,
        context_id: i64,
        customer_id: Option<i64>,
        title: &str,
    ) -> Result<Case> {
        let id = {
            let conn = self.conn();
            let id = new_case(&conn, context_id, customer_id, title)?;
            conn.execute(
                "UPDATE cases SET started = ?1 WHERE id = ?2",
                params![customer_id.is_some(), id],
            )?;
            id
        };
        self.case(id)
    }

    /// Names a Vorgang anew, never blank
    pub fn set_case_title(&self, id: i64, title: &str) -> Result<Case> {
        let changed = self.conn().execute(
            "UPDATE cases SET title = ?1 WHERE id = ?2",
            params![named(title)?, id],
        )?;
        if changed == 0 {
            bail!("unknown Vorgang");
        }
        self.case(id)
    }

    /// Removes a Vorgang that has no documents (one that has goes with its last draft)
    pub fn delete_case(&self, id: i64) -> Result<()> {
        if !drop_if_empty(&self.conn(), id)? {
            bail!("only a Vorgang without documents can be removed");
        }
        Ok(())
    }

    /// Where the numbers of a context's Vorgänge stand today
    pub fn case_sequence(&self, context_id: i64, settings: &Settings) -> Result<Sequence> {
        let today = Date::parse(&crate::utils::today())?;
        sequence(&self.conn(), context_id, settings, today)
    }

    /// Makes today's period of a context's Vorgänge continue from `next`, as `set_counter` does for
    /// documents: refused at or below the highest counter given, dropped when it is what follows
    pub fn set_case_counter(
        &self,
        context_id: i64,
        settings: &Settings,
        next: u32,
    ) -> Result<Sequence> {
        let today = Date::parse(&crate::utils::today())?;
        let conn = self.conn();
        let calendar = settings.calendar();
        let reset = settings.cases.number_reset;
        let period = calendar.period(reset, today);
        let given = given(&conn, context_id)?;
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
                params![context_id, FLOOR_KIND, period.key()],
            )?;
        } else {
            conn.execute(
                "INSERT INTO number_floors (context_id, kind, period, next, set_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))
                 ON CONFLICT (context_id, kind, period)
                 DO UPDATE SET next = excluded.next, set_at = excluded.set_at",
                params![context_id, FLOOR_KIND, period.key(), next],
            )?;
        }
        sequence(&conn, context_id, settings, today)
    }

    /// The history of all documents of a Vorgang together, as recorded
    pub fn case_events(&self, id: i64) -> Result<Vec<Event>> {
        let documents: Vec<i64> = {
            let conn = self.conn();
            let mut statement = conn.prepare("SELECT id FROM invoices WHERE case_id = ?1")?;
            let ids = statement
                .query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        };
        let mut events = documents
            .into_iter()
            .map(|doc| self.events(doc))
            .collect::<Result<Vec<_>>>()?
            .concat();
        events.sort_by(|a, b| (&a.at, a.id).cmp(&(&b.at, b.id)));
        Ok(events)
    }

    /// The number of a document's Vorgang, for `{vorgang_number}`
    pub fn case_number_of(&self, invoice_id: i64) -> Result<String> {
        let conn = self.conn();
        let case = case_of(&conn, invoice_id)?;
        Ok(
            conn.query_row("SELECT number FROM cases WHERE id = ?1", [case], |r| {
                r.get(0)
            })?,
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::{InvoiceFiles, StoredFile};
    use crate::store::Sequence;

    /// A draft for a customer by id
    fn for_customer(id: Option<i64>) -> InvoiceDraft {
        let mut draft = draft();
        draft.customer.id = id;
        draft
    }

    pub(crate) fn bake(store: &Store, id: i64, number: &str, total: &str) {
        let file = StoredFile {
            path: "x.pdf".into(),
            sha256: "0".repeat(64),
        };
        let sequence = Sequence {
            next_number: number.into(),
            counter: 1,
            highest: None,
            floor: None,
        };
        let files = InvoiceFiles {
            pdf: file,
            xml: None,
            regenerated: false,
        };
        store
            .finalize(id, &sequence, &settings(), None, total, &files)
            .unwrap();
    }

    fn year() -> String {
        crate::utils::today()[..4].to_owned()
    }

    #[test]
    fn a_plain_document_makes_its_own_numbered_vorgang() {
        let store = Store::in_memory().unwrap();
        let first = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        let second = store
            .save_draft(None, 1, Kind::Invoice, &for_customer(Some(8)), None)
            .unwrap();

        assert_ne!(first.case_id, second.case_id);
        let case = store.case(first.case_id).unwrap();
        assert_eq!(case.number, format!("VG-{}-001", year()));
        assert_eq!(case.customer_id, Some(7));
        assert_eq!(case.customer_name, draft().customer.name);
        assert_eq!(case.documents, 1);
        assert!(!case.locked);
        assert_eq!(case.stage, CaseStage::DraftingOffer);
        assert_eq!(
            store.case(second.case_id).unwrap().number,
            format!("VG-{}-002", year())
        );
        let listed = store.invoices(None, None, false).unwrap();
        assert!(listed.iter().all(|e| e.case_id > 0));
        assert_eq!(
            store.case_number_of(second.id).unwrap(),
            format!("VG-{}-002", year())
        );
    }

    #[test]
    fn the_customer_follows_a_lone_draft_and_is_locked_after() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        // alone and a draft: the Vorgang takes the new customer
        store
            .save_draft(Some(offer.id), 1, Kind::Offer, &for_customer(Some(9)), None)
            .unwrap();
        assert_eq!(store.case(offer.case_id).unwrap().customer_id, Some(9));

        // a second document locks it, and must be for the same customer
        assert!(store
            .save_draft_in_case(offer.case_id, Kind::Offer, &for_customer(Some(7)), None)
            .is_err());
        let nachtrag = store
            .save_draft_in_case(offer.case_id, Kind::Offer, &for_customer(Some(9)), None)
            .unwrap();
        assert_eq!(nachtrag.case_id, offer.case_id);
        assert!(store.case(offer.case_id).unwrap().locked);
        assert!(store
            .save_draft(Some(offer.id), 1, Kind::Offer, &for_customer(Some(7)), None)
            .is_err());

        // a created document alone locks it as well
        let lone = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        bake(&store, lone.id, "AN-2026-001", "100");
        assert!(store.case(lone.case_id).unwrap().locked);
    }

    #[test]
    fn a_locked_vorgang_without_customer_lets_its_drafts_choose_one() {
        let store = Store::in_memory().unwrap();
        let first = store
            .save_draft(None, 1, Kind::Offer, &for_customer(None), None)
            .unwrap();
        let second = store.duplicate_to(first.id, "2026-12-01", true).unwrap();
        assert!(store.case(first.case_id).unwrap().locked);

        // the first to choose gives the Vorgang its customer
        store
            .save_draft(Some(first.id), 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        assert_eq!(store.case(first.case_id).unwrap().customer_id, Some(7));
        // the other one, still without, can be saved until it takes that customer
        store
            .save_draft(Some(second.id), 1, Kind::Offer, &for_customer(None), None)
            .unwrap();
        assert!(store
            .save_draft(
                Some(second.id),
                1,
                Kind::Offer,
                &for_customer(Some(8)),
                None
            )
            .is_err());
    }

    #[test]
    fn a_created_document_says_so_before_its_vorgang_refuses_a_customer() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        bake(&store, offer.id, "AN-2026-001", "100");
        let error = store
            .save_draft(Some(offer.id), 1, Kind::Offer, &for_customer(Some(8)), None)
            .unwrap_err();
        assert!(error.to_string().contains("exported"), "{error}");
        // an unknown draft is no draft, not a document without Vorgang
        let error = store.delete_draft(999).unwrap_err();
        assert!(error.to_string().contains("only drafts"), "{error}");
    }

    #[test]
    fn a_deleted_customer_leaves_its_vorgang_open_for_another() {
        let store = Store::in_memory().unwrap();
        let customer = store
            .save_customer(&crate::draft::fixtures::customer())
            .unwrap()
            .id;
        let started = store.create_case(1, customer, "Bad").unwrap();
        store.delete_customer(customer.unwrap()).unwrap();

        assert_eq!(store.case(started.id).unwrap().customer_id, None);
        store
            .save_draft_in_case(started.id, Kind::Offer, &for_customer(Some(9)), None)
            .unwrap();
        assert_eq!(store.case(started.id).unwrap().customer_id, Some(9));
    }

    #[test]
    fn copies_go_where_they_are_asked_and_an_emptied_vorgang_goes() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        let alone = store.duplicate_to(offer.id, "2026-12-01", false).unwrap();
        let joined = store.duplicate_to(offer.id, "2026-12-01", true).unwrap();
        assert_ne!(alone.case_id, offer.case_id);
        // a new document's Vorgang is called after it, a copy's like the original's
        let named = store.case(offer.case_id).unwrap().title;
        assert!(named.starts_with("Angebot vom "), "{named}");
        assert_eq!(store.case(alone.case_id).unwrap().title, named);
        assert_eq!(joined.case_id, offer.case_id);

        bake(&store, offer.id, "AN-2026-001", "100");
        let invoice = store
            .invoice_from_offer(offer.id, "2026-12-01", &settings())
            .unwrap();
        assert_eq!(invoice.case_id, offer.case_id);

        // the last draft takes its Vorgang along, others stay
        store.delete_draft(alone.id).unwrap();
        assert!(store.case(alone.case_id).is_err());
        store.delete_draft(joined.id).unwrap();
        assert!(store.case(offer.case_id).is_ok());
    }

    #[test]
    fn a_vorgang_can_be_started_empty_named_and_removed_while_empty() {
        let store = Store::in_memory().unwrap();
        assert!(store.create_case(1, Some(7), "  ").is_err());
        let started = store.create_case(1, Some(7), " Badsanierung ").unwrap();
        assert_eq!(started.title, "Badsanierung");
        assert_eq!(started.documents, 0);
        assert!(!started.archived);
        assert_eq!(store.cases(Some(1), false).unwrap().len(), 1);
        assert_eq!(store.cases(Some(2), false).unwrap().len(), 0);

        assert!(store.set_case_title(started.id, "  ").is_err());
        let renamed = store.set_case_title(started.id, " Bad ").unwrap();
        assert_eq!(renamed.title, "Bad");

        // started for its customer, it is locked before it has a document
        assert!(started.locked);
        assert!(store
            .save_draft_in_case(started.id, Kind::Offer, &for_customer(Some(8)), None)
            .is_err());
        let doc = store
            .save_draft_in_case(started.id, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        assert!(store
            .save_draft(Some(doc.id), 1, Kind::Offer, &for_customer(Some(8)), None)
            .is_err());
        assert!(store.delete_case(started.id).is_err());
        store.delete_draft(doc.id).unwrap();
        // emptied by its last draft, it goes
        assert!(store.case(started.id).is_err());

        let empty = store.create_case(1, None, "Leer").unwrap();
        store.delete_case(empty.id).unwrap();
        assert!(store.cases(None, true).unwrap().is_empty());
    }

    #[test]
    fn what_is_due_and_what_is_left_to_bill_tell_where_the_customer_is() {
        let doc = |kind: Kind, total: &str, paid: &str, answer: Option<Answer>| Standing {
            kind,
            status: Status::Exported,
            total: Some(total.into()),
            paid: crate::history::decimal(paid),
            answer,
            relation: None,
            lapsed: false,
            superseded: false,
            settled: false,
            due: (kind == Kind::Invoice).then(|| "2026-11-10".into()),
        };
        let offer = doc(Kind::Offer, "1000", "0", Some(Answer::Accepted));
        let stage = |docs: &[Standing], today: &str| status(docs, today).0;
        // an Abschlag of 300 on it, open, then due, then paid
        let open = [offer.clone(), doc(Kind::Invoice, "300", "0", None)];
        assert_eq!(stage(&open, "2026-11-10"), CaseStage::Invoiced);
        assert_eq!(stage(&open, "2026-11-11"), CaseStage::Overdue);
        let paid = [offer.clone(), doc(Kind::Invoice, "300", "300", None)];
        assert_eq!(stage(&paid, "2026-11-11"), CaseStage::PartlyInvoiced);
        // the Schlussrechnung for the rest, paid
        let done = [
            offer,
            doc(Kind::Invoice, "300", "300", None),
            doc(Kind::Invoice, "700", "700", None),
        ];
        assert_eq!(stage(&done, "2026-12-01"), CaseStage::Paid);
    }

    #[test]
    fn a_vorgang_stands_at_its_furthest_step() {
        let store = Store::in_memory().unwrap();
        store
            .save_context(&crate::context::Context::new(settings(), ""), None, None)
            .unwrap();
        let started = store.create_case(1, Some(7), "Bad").unwrap();
        assert_eq!(started.stage, CaseStage::New);
        let stage = || store.case(started.id).unwrap().stage;

        store
            .save_draft_in_case(started.id, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        assert_eq!(stage(), CaseStage::DraftingOffer);
        // dated today, so not due yet
        let mut written = for_customer(Some(7));
        written.issue_date = crate::utils::today();
        let invoice = store
            .save_draft_in_case(started.id, Kind::Invoice, &written, None)
            .unwrap();
        assert_eq!(stage(), CaseStage::DraftingInvoice);

        // an invoice taken back bills nothing: neither paid nor open, cancelled
        bake(&store, invoice.id, "RE-2026-001", "100");
        assert_eq!(stage(), CaseStage::Invoiced);
        let storno = store.cancel_invoice(invoice.id, "2026-11-02").unwrap();
        bake(&store, storno.id, "RE-2026-002", "-100");
        assert_eq!(stage(), CaseStage::Cancelled);
    }

    #[test]
    fn where_a_vorgang_stands_comes_from_its_documents() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        let case = offer.case_id;
        bake(&store, offer.id, "AN-2026-001", "1000");
        assert_eq!(store.case(case).unwrap().stage, CaseStage::Offered);
        assert_eq!(store.case(case).unwrap().offered, "1000");

        store
            .set_answer(offer.id, Some((Answer::Declined, "2026-10-20")))
            .unwrap();
        let declined = store.case(case).unwrap();
        assert_eq!(
            (declined.stage, declined.offered.as_str()),
            (CaseStage::Declined, "0")
        );
        store
            .set_answer(offer.id, Some((Answer::Accepted, "2026-10-21")))
            .unwrap();
        assert_eq!(store.case(case).unwrap().stage, CaseStage::Accepted);
        assert_eq!(store.case(case).unwrap().accepted, "1000");

        // dated today, so not due yet
        let today = crate::utils::today();
        let first = store
            .invoice_from_offer(offer.id, &today, &settings())
            .unwrap();
        bake(&store, first.id, "RE-2026-001", "400");
        // the offer is billed in full, a second invoice bills something else of the Vorgang
        let mut other = draft();
        other.issue_date.clone_from(&today);
        let second = store
            .save_draft_in_case(case, Kind::Invoice, &other, None)
            .unwrap();
        bake(&store, second.id, "RE-2026-002", "600.50");
        let invoiced = store.case(case).unwrap();
        assert_eq!(invoiced.stage, CaseStage::Invoiced);
        assert_eq!(
            (invoiced.invoiced.as_str(), invoiced.open.as_str()),
            ("1000.5", "1000.5")
        );

        store.add_payment(first.id, "400", "2026-10-25").unwrap();
        let partly = store.case(case).unwrap();
        assert_eq!(partly.stage, CaseStage::PartlyPaid);
        assert_eq!(
            (partly.paid.as_str(), partly.open.as_str()),
            ("400", "600.5")
        );
        store.set_paid(second.id, Some("2026-10-26")).unwrap();
        assert_eq!(store.case(case).unwrap().stage, CaseStage::Paid);
        assert_eq!(store.case(case).unwrap().open, "0");

        // archived when all its documents are
        for id in [offer.id, first.id, second.id] {
            store.archive_invoice(id).unwrap();
        }
        assert!(store.case(case).unwrap().archived);
        assert!(store.cases(None, false).unwrap().is_empty());
        assert_eq!(store.cases(None, true).unwrap().len(), 1);
    }

    #[test]
    fn the_history_of_a_vorgang_is_that_of_its_documents_together() {
        let store = Store::in_memory().unwrap();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &for_customer(Some(7)), None)
            .unwrap();
        bake(&store, offer.id, "AN-2026-001", "100");
        let invoice = store
            .invoice_from_offer(offer.id, "2026-10-22", &settings())
            .unwrap();

        let events = store.case_events(offer.case_id).unwrap();
        let documents: Vec<_> = events.iter().map(|e| e.invoice_id).collect();
        assert_eq!(documents, [offer.id, offer.id, invoice.id]);
    }

    #[test]
    fn vorgaenge_are_numbered_by_the_settings_of_their_context() {
        let store = Store::in_memory().unwrap();
        let mut own = settings();
        own.cases.number_pattern = "[V]NNNN".into();
        let context = store
            .save_context(&crate::context::Context::new(own.clone(), ""), None, None)
            .unwrap()
            .id
            .unwrap();
        let doc = store
            .save_draft(None, context, Kind::Offer, &draft(), None)
            .unwrap();
        assert_eq!(store.case(doc.case_id).unwrap().number, "V0001");
        assert_eq!(
            store.case_sequence(context, &own).unwrap().next_number,
            "V0002"
        );
        // a pattern that does not parse numbers by the default
        own.cases.number_pattern = "[kaputt".into();
        assert!(store
            .case_sequence(context, &own)
            .unwrap()
            .next_number
            .starts_with("VG-"));
    }

    #[test]
    fn the_counter_of_the_vorgaenge_can_continue_from_another_program() {
        let store = Store::in_memory().unwrap();
        let own = settings();
        store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap();
        let first = format!("VG-{}-001", year());
        assert_eq!(
            store.case_sequence(1, &own).unwrap().highest,
            Some(first.clone())
        );

        assert!(store.set_case_counter(1, &own, 1).is_err());
        let set = store.set_case_counter(1, &own, 40).unwrap();
        assert_eq!(set.next_number, format!("VG-{}-040", year()));
        assert_eq!(set.floor, Some(40));
        let doc = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap();
        assert_eq!(
            store.case(doc.case_id).unwrap().number,
            format!("VG-{}-040", year())
        );
        // the documents' counters are their own
        assert_eq!(
            store
                .sequence(
                    1,
                    Kind::Offer,
                    &own,
                    Date::parse(&crate::utils::today()).unwrap()
                )
                .unwrap()
                .floor,
            None
        );
        // setting what follows anyway drops the floor
        assert_eq!(store.set_case_counter(1, &own, 41).unwrap().floor, None);
    }

    #[test]
    fn a_version_twelve_database_puts_linked_documents_of_a_customer_together() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::migrate(&conn).unwrap();
        conn.execute_batch(
            "DROP TABLE cases; DROP INDEX invoices_case; ALTER TABLE invoices DROP COLUMN case_id;
             CREATE TABLE document_links (
                 offer_id INTEGER NOT NULL, invoice_id INTEGER NOT NULL, created_at TEXT NOT NULL,
                 PRIMARY KEY (offer_id, invoice_id)
             );
             PRAGMA user_version = 12;",
        )
        .unwrap();
        let insert = |id: i64, kind: &str, customer: Option<i64>, day: &str| {
            let mut draft = for_customer(customer);
            draft.issue_date = day.into();
            conn.execute(
                "INSERT INTO invoices (id, context_id, kind, status, draft, updated_at)
                 VALUES (?1, 1, ?2, 'draft', ?3, '2026-10-01')",
                params![id, kind, serde_json::to_string(&draft).unwrap()],
            )
            .unwrap();
        };
        insert(1, "offer", Some(7), "2026-03-01");
        insert(2, "invoice", Some(7), "2026-04-01");
        insert(3, "invoice", Some(8), "2026-04-02");
        insert(4, "offer", Some(9), "2026-01-15");
        conn.execute_batch(
            "INSERT INTO document_links VALUES (1, 2, '2026-04-01'), (1, 3, '2026-04-02');",
        )
        .unwrap();

        let store = Store::from_connection(conn).unwrap();

        let case = |id| store.invoice(id).unwrap().case_id;
        assert_eq!(case(1), case(2));
        // linked, but for another customer
        assert_ne!(case(1), case(3));
        let number = |id| store.case(case(id)).unwrap().number;
        // numbered in the order of their first issue date
        assert_eq!(number(4), "VG-2026-001");
        assert_eq!(number(1), "VG-2026-002");
        assert_eq!(number(3), "VG-2026-003");
        assert_eq!(store.case(case(1)).unwrap().customer_id, Some(7));
        // unnamed before, called after the first document
        assert_eq!(store.case(case(1)).unwrap().title, "Angebot vom 01.03.2026");
    }
}
