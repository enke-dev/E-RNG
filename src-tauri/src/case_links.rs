//! Vorgänge that refer to each other; they never merge (two numbers, two locked customers, two
//! histories cannot become one). The reference is directed, from the source to its follower: a
//! Folgevorgang follows the one its Folgeangebot was made of (`follow`), a Vorgang made for a
//! Sammelrechnung collects the ones whose offers it bills (`collect`). Every document stays in
//! exactly one Vorgang; an invoice of a collecting Vorgang bills positions of the offers of the
//! Vorgänge it collects.

use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::billing::OfferBilling;
use crate::cases::new_case;
use crate::draft::{build_invoice, totals};
use crate::history::decimal;
use crate::model::{Kind, Position, PositionSource, Status, StoredInvoice};
use crate::store::{insert_draft, now, Store};

/// How a Vorgang refers to another
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaseLink {
    /// A Folgevorgang: a follow-up deal, made from an offer of the source
    Follow,
    /// A Vorgang of a Sammelrechnung over the offers of its sources
    Collect,
}

impl CaseLink {
    fn as_str(self) -> &'static str {
        match self {
            Self::Follow => "follow",
            Self::Collect => "collect",
        }
    }

    fn from_column(text: &str) -> Self {
        if text == "collect" {
            Self::Collect
        } else {
            Self::Follow
        }
    }
}

/// Another Vorgang as a reference names it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseRef {
    pub id: i64,
    pub number: String,
    pub link: CaseLink,
}

/// The references between Vorgänge
pub(crate) fn migrate_to_case_links(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS case_links (
             source_id INTEGER NOT NULL,
             follower_id INTEGER NOT NULL,
             kind TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (source_id, follower_id)
         );
         CREATE INDEX IF NOT EXISTS case_links_follower ON case_links (follower_id);
         PRAGMA user_version = 16;",
    )?;
    Ok(())
}

/// A Vorgang that went takes its references along; with `None` every reference to a Vorgang that
/// is gone (a removed context)
pub(crate) fn forget(conn: &Connection, case_id: Option<i64>) -> Result<()> {
    match case_id {
        Some(id) => conn.execute(
            "DELETE FROM case_links WHERE source_id = ?1 OR follower_id = ?1",
            [id],
        )?,
        None => conn.execute(
            "DELETE FROM case_links WHERE source_id NOT IN (SELECT id FROM cases)
                 OR follower_id NOT IN (SELECT id FROM cases)",
            [],
        )?,
    };
    Ok(())
}

/// The Vorgänge a Vorgang refers to (`sources`) or that refer to it (`followers`), oldest first
pub(crate) fn refs(conn: &Connection, case_id: i64, sources: bool) -> Result<Vec<CaseRef>> {
    let sql = if sources {
        "SELECT c.id, c.number, l.kind FROM case_links l JOIN cases c ON c.id = l.source_id
         WHERE l.follower_id = ?1 ORDER BY c.id"
    } else {
        "SELECT c.id, c.number, l.kind FROM case_links l JOIN cases c ON c.id = l.follower_id
         WHERE l.source_id = ?1 ORDER BY c.id"
    };
    let mut statement = conn.prepare(sql)?;
    let rows = statement
        .query_map([case_id], |r| {
            Ok(CaseRef {
                id: r.get(0)?,
                number: r.get(1)?,
                link: CaseLink::from_column(&r.get::<_, String>(2)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Whether the positions of an invoice in `case_id` may bill offers of `offer_case`: its own
/// Vorgang, or one it collects
pub(crate) fn bills(conn: &Connection, case_id: i64, offer_case: i64) -> Result<bool> {
    if case_id == offer_case {
        return Ok(true);
    }
    Ok(conn.query_row(
        "SELECT COUNT(*) > 0 FROM case_links
         WHERE follower_id = ?1 AND source_id = ?2 AND kind = 'collect'",
        params![case_id, offer_case],
        |r| r.get(0),
    )?)
}

fn link(conn: &Connection, source: i64, follower: i64, kind: CaseLink) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO case_links (source_id, follower_id, kind, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![source, follower, kind.as_str(), now()],
    )?;
    Ok(())
}

impl Store {
    /// A Folgeangebot: a copy of a created offer, dated `today`, in a new Vorgang that follows the
    /// offer's
    pub fn follow_offer(&self, offer_id: i64, today: &str) -> Result<StoredInvoice> {
        let offer = self.invoice(offer_id)?;
        if offer.kind != Kind::Offer || offer.status != Status::Exported {
            bail!("Ein Folgeangebot gibt es nur zu einem erstellten Angebot.");
        }
        let copy = self.duplicate_to(offer_id, today, false)?;
        link(&self.conn(), offer.case_id, copy.case_id, CaseLink::Follow)?;
        Ok(copy)
    }

    /// A Sammelrechnung over Vorgänge of one customer: a new Vorgang named `title` for the customer
    /// that collects them, and in it an invoice draft dated `today` with all that is still open of
    /// their offers
    pub fn collect(&self, case_ids: &[i64], today: &str, title: &str) -> Result<StoredInvoice> {
        crate::cases::named(title)?;
        let cases = case_ids
            .iter()
            .map(|&id| self.case(id))
            .collect::<Result<Vec<_>>>()?;
        let Some(first) = cases.first() else {
            bail!("Eine Sammelrechnung braucht Vorgänge.");
        };
        if cases.len() < 2 {
            bail!("Eine Sammelrechnung fasst mindestens zwei Vorgänge zusammen.");
        }
        if cases
            .iter()
            .any(|c| c.sources.iter().any(|s| s.link == CaseLink::Collect))
        {
            bail!("Ein Vorgang, der andere zusammenfasst, wird nicht noch einmal gesammelt.");
        }
        if first.customer_id.is_none()
            || cases
                .iter()
                .any(|c| c.customer_id != first.customer_id || c.context_id != first.context_id)
        {
            bail!("Eine Sammelrechnung fasst Vorgänge desselben Kunden zusammen.");
        }
        let billings: Vec<OfferBilling> = case_ids
            .iter()
            .map(|&id| self.case_billing(id, None))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        let positions: Vec<Position> = billings
            .iter()
            .flat_map(|billing| {
                billing
                    .positions
                    .iter()
                    .filter(|b| decimal(&b.open) > Decimal::ZERO)
                    .map(|b| {
                        let mut position = b.position.clone();
                        if position.quantity.is_some() || decimal(&b.open) != Decimal::ONE {
                            position.quantity = Some(b.open.clone());
                        }
                        position.source = Some(PositionSource {
                            offer_id: billing.offer_id,
                            index: b.index,
                        });
                        position
                    })
            })
            .collect();
        if positions.is_empty() {
            bail!("In diesen Vorgängen ist nichts mehr offen.");
        }
        let settings = self.settings(first.context_id)?;
        let customer = self.invoice(billings[0].offer_id)?.draft.customer;
        let mut draft = settings.blank_draft(Kind::Invoice, today);
        draft.buyer_reference.clone_from(&customer.buyer_reference);
        draft.customer = customer;
        draft.positions = positions;
        let total = build_invoice(&draft, &settings, "ENTWURF")
            .ok()
            .map(|invoice| totals(&invoice).due);
        let id = {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            // started for its customer, like `Neuer Vorgang`
            let case = new_case(&tx, first.context_id, first.customer_id, title)?;
            tx.execute("UPDATE cases SET started = 1 WHERE id = ?1", [case])?;
            for &source in case_ids {
                link(&tx, source, case, CaseLink::Collect)?;
            }
            let json = serde_json::to_string(&draft)?;
            let id = insert_draft(
                &tx,
                first.context_id,
                Kind::Invoice,
                &json,
                total.as_deref(),
                None,
                case,
            )?;
            tx.commit()?;
            id
        };
        self.invoice(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::tests::bake;
    use crate::context::Context;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::CaseStage;

    fn store() -> Store {
        let store = Store::in_memory().unwrap();
        store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        store
    }

    /// A created offer for a customer, in a Vorgang of its own
    fn offer(store: &Store, customer: i64, number: &str) -> StoredInvoice {
        let mut written = draft();
        written.customer.id = Some(customer);
        let saved = store
            .save_draft(None, 1, Kind::Offer, &written, None)
            .unwrap();
        bake(store, saved.id, number, "100");
        store.invoice(saved.id).unwrap()
    }

    #[test]
    fn a_folgeangebot_starts_a_vorgang_that_follows() {
        let store = store();
        let first = offer(&store, 7, "AN-1");
        let follow = store.follow_offer(first.id, "2026-11-02").unwrap();
        assert_ne!(follow.case_id, first.case_id);
        assert_eq!(follow.status, Status::Draft);
        assert_eq!(follow.draft.positions, first.draft.positions);
        let source = store.case(first.case_id).unwrap();
        let follower = store.case(follow.case_id).unwrap();
        assert_eq!(source.followers[0].id, follower.id);
        assert_eq!(source.followers[0].link, CaseLink::Follow);
        assert_eq!(follower.sources[0].number, source.number);
        // only of a created offer
        assert!(store.follow_offer(follow.id, "2026-11-02").is_err());

        // a Vorgang that goes takes its references
        store.delete_draft(follow.id).unwrap();
        assert!(store.case(first.case_id).unwrap().followers.is_empty());
    }

    #[test]
    fn a_sammelrechnung_collects_vorgaenge_of_one_customer() {
        let store = store();
        let a = offer(&store, 7, "AN-1");
        let b = offer(&store, 7, "AN-2");
        let other = offer(&store, 8, "AN-3");
        assert!(store.collect(&[a.case_id], "2026-11-02", "Sammel").is_err());
        assert!(store
            .collect(&[a.case_id, other.case_id], "2026-11-02", "Sammel")
            .is_err());

        let invoice = store
            .collect(&[a.case_id, b.case_id], "2026-11-02", "Sammel")
            .unwrap();
        let sources: Vec<_> = invoice
            .draft
            .positions
            .iter()
            .map(|p| p.source.map(|s| s.offer_id))
            .collect();
        // every position of both offers, the fixture has two each
        assert_eq!(sources, [Some(a.id), Some(a.id), Some(b.id), Some(b.id)]);
        let collecting = store.case(invoice.case_id).unwrap();
        assert!(collecting.locked);
        assert_eq!(
            collecting
                .sources
                .iter()
                .map(|s| (s.id, s.link))
                .collect::<Vec<_>>(),
            [
                (a.case_id, CaseLink::Collect),
                (b.case_id, CaseLink::Collect)
            ]
        );
        // its invoice bills the offers of the collected Vorgänge, and is saved like that
        store
            .save_draft(Some(invoice.id), 1, Kind::Invoice, &invoice.draft, None)
            .unwrap();
        assert_eq!(store.case_billing(invoice.case_id, None).unwrap().len(), 2);
        assert_eq!(store.offer_numbers(invoice.id).unwrap(), ["AN-1", "AN-2"]);

        // the collecting Vorgang is not collected again
        let c = offer(&store, 7, "AN-4");
        assert!(store
            .collect(&[invoice.case_id, c.case_id], "2026-11-02", "Sammel")
            .is_err());

        // once created, nothing is open any more
        bake(&store, invoice.id, "RE-1", "200");
        assert!(store
            .collect(&[a.case_id, b.case_id], "2026-11-03", "Sammel")
            .is_err());
        // and each collected Vorgang stands as its part of the invoice says, half of it here
        let collected = store.case(a.case_id).unwrap();
        assert_eq!(
            (
                collected.stage,
                collected.invoiced.as_str(),
                collected.documents
            ),
            (CaseStage::Invoiced, "100", 1)
        );
        store.add_payment(invoice.id, "200", "2026-11-04").unwrap();
        let collected = store.case(b.case_id).unwrap();
        assert_eq!(
            (collected.stage, collected.open.as_str()),
            (CaseStage::Paid, "0")
        );
        assert_eq!(store.case(invoice.case_id).unwrap().invoiced, "200");
    }
}
