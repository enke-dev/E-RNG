//! What invoices bill of offers. An invoice position may refer to a position of a created offer of
//! its Vorgang (`Position.source`, the offer and the place of the position in it; a created offer
//! never changes, so the place stays). It bills its own quantity of it, for good: what is open of
//! an offer position is its quantity less what created invoices billed. Which offers an invoice
//! bills follows from its positions, there is no table of links.
//!
//! A Rechnung bills everything that is open of an offer, a Teilrechnung some positions or less of
//! them, a Sammelrechnung positions of several offers of its Vorgang; all of them bill for good.

use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::draft::{build_invoice, totals};
use crate::history::decimal;
use crate::model::{
    Answer, InvoiceDraft, Kind, Linked, Position, PositionSource, Settings, Status, StoredInvoice,
};
use crate::store::{insert_draft, Store};

/// What is billed of a position of an offer
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PositionBilling {
    /// Its place in the offer
    pub index: usize,
    pub position: Position,
    /// By created invoices, a decimal like the quantities
    pub billed: String,
    /// By invoice drafts, which do not bill yet
    pub drafted: String,
    /// What created invoices have not billed yet, never below zero
    pub open: String,
}

/// What is billed of a created offer, position by position, and by which invoices
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfferBilling {
    pub offer_id: i64,
    pub number: String,
    pub positions: Vec<PositionBilling>,
    /// The invoices that bill any of it, drafts too, newest first
    pub invoices: Vec<Linked>,
}

/// How many of a position: its quantity as typed (`2,5` as well as `2.5`), a blank one is one
pub(crate) fn quantity(position: &Position) -> Decimal {
    position
        .quantity
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map_or(Decimal::ONE, |q| {
            crate::draft::decimal(q).unwrap_or_default()
        })
}

fn text(value: Decimal) -> String {
    value.normalize().to_string()
}

/// One invoice position that refers to an offer position
struct Reference {
    invoice_id: i64,
    created: bool,
    offer_id: i64,
    index: usize,
    quantity: Decimal,
}

/// The references of all invoice positions, or of those to one offer
fn references(conn: &Connection, offer: Option<i64>) -> Result<Vec<Reference>> {
    let mut statement = conn.prepare(
        "SELECT i.id, i.status, json_extract(p.value, '$.source.offer_id'),
             json_extract(p.value, '$.source.index'), json_extract(p.value, '$.quantity')
         FROM invoices i, json_each(i.draft, '$.positions') p
         WHERE i.kind = 'invoice' AND json_extract(p.value, '$.source.offer_id') IS NOT NULL
             AND (?1 IS NULL OR json_extract(p.value, '$.source.offer_id') = ?1)",
    )?;
    let rows = statement.query_map([offer], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (invoice_id, status, offer_id, index, amount) = row?;
        Ok(Reference {
            invoice_id,
            created: status == "exported",
            offer_id,
            index: usize::try_from(index).unwrap_or(usize::MAX),
            quantity: quantity(&Position {
                quantity: amount,
                ..Position::default()
            }),
        })
    })
    .collect()
}

/// For every document with references, how many documents of the other kind it is billed with:
/// an invoice the offers it bills, an offer the invoices that bill it
pub(crate) fn reference_counts(conn: &Connection) -> Result<HashMap<i64, u32>> {
    let pairs: std::collections::BTreeSet<(i64, i64)> = references(conn, None)?
        .into_iter()
        .map(|r| (r.invoice_id, r.offer_id))
        .collect();
    Ok(pairs
        .into_iter()
        .flat_map(|(invoice, offer)| [invoice, offer])
        .fold(HashMap::new(), |mut counts, id| {
            *counts.entry(id).or_insert(0) += 1;
            counts
        }))
}

/// A document as the other end of a reference names it
fn linked(conn: &Connection, id: i64) -> Result<Linked> {
    let (kind, number, status, draft): (String, Option<String>, String, String) = conn.query_row(
        "SELECT kind, number, status, draft FROM invoices WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
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
}

/// The created offers that `positions` refer to, by id: what an invoice that names its offers names
fn numbers_of(conn: &Connection, positions: &[Position]) -> Result<Vec<String>> {
    let offers: std::collections::BTreeSet<i64> = positions
        .iter()
        .filter_map(|p| p.source.map(|s| s.offer_id))
        .collect();
    offers
        .into_iter()
        .map(|id| -> Result<Option<String>> {
            // a missing offer or one without number names nothing
            Ok(conn
                .query_row(
                    "SELECT number FROM invoices WHERE id = ?1 AND kind = 'offer'",
                    [id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten())
        })
        .filter_map(Result::transpose)
        .collect()
}

/// The positions of an invoice may only refer to positions that exist on created offers of its
/// Vorgang; an offer's positions refer to nothing
pub(crate) fn check_sources(
    conn: &Connection,
    case_id: i64,
    kind: Kind,
    draft: &InvoiceDraft,
) -> Result<()> {
    for source in draft.positions.iter().filter_map(|p| p.source) {
        if kind != Kind::Invoice {
            bail!("Nur die Positionen einer Rechnung beziehen sich auf ein Angebot.");
        }
        let offer: Option<(String, String, i64, String)> = conn
            .query_row(
                "SELECT kind, status, case_id, draft FROM invoices WHERE id = ?1",
                [source.offer_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((offer_kind, status, offer_case, offer_draft)) = offer else {
            bail!("Das Angebot einer Position gibt es nicht.");
        };
        if offer_kind != "offer"
            || status != "exported"
            || !crate::case_links::bills(conn, case_id, offer_case)?
        {
            bail!(
                "Eine Position bezieht sich nur auf ein erstelltes Angebot desselben Vorgangs \
                 oder eines, den er zusammenfasst."
            );
        }
        let offer_draft: InvoiceDraft = serde_json::from_str(&offer_draft)?;
        if source.index >= offer_draft.positions.len() {
            bail!("Die Position des Angebots gibt es nicht.");
        }
    }
    Ok(())
}

/// The links of older versions become references: each position of a linked invoice refers to the
/// first position of the offer that is not taken yet with the same description, one with the same
/// price first. Only within a Vorgang; positions without a match stay as they are.
pub(crate) fn migrate_to_sources(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let has_links: bool = tx.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'document_links'",
        [],
        |r| r.get(0),
    )?;
    if has_links {
        let links = {
            let mut statement = tx.prepare(
                "SELECT l.offer_id, l.invoice_id, o.draft, i.draft
                 FROM document_links l
                 JOIN invoices o ON o.id = l.offer_id AND o.status = 'exported'
                 JOIN invoices i ON i.id = l.invoice_id AND i.case_id = o.case_id
                 ORDER BY l.invoice_id, l.offer_id",
            )?;
            let rows = statement
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        // an invoice linked to several offers is read and written once with all of them
        let mut invoices: BTreeMap<i64, InvoiceDraft> = BTreeMap::new();
        for (offer_id, invoice_id, offer, invoice) in links {
            let offer: InvoiceDraft = serde_json::from_str(&offer)?;
            let draft = match invoices.entry(invoice_id) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => {
                    e.insert(serde_json::from_str(&invoice)?)
                }
            };
            let mut taken = vec![false; offer.positions.len()];
            let same = |a: &str, b: &str| a.trim() == b.trim();
            for position in draft.positions.iter_mut().filter(|p| p.source.is_none()) {
                let found = offer
                    .positions
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| !taken[*at])
                    .filter(|(_, o)| same(&o.description, &position.description))
                    .min_by_key(|(_, o)| !same(&o.price, &position.price))
                    .map(|(at, _)| at);
                if let Some(at) = found {
                    taken[at] = true;
                    position.source = Some(PositionSource {
                        offer_id,
                        index: at,
                    });
                }
            }
        }
        for (id, draft) in invoices {
            tx.execute(
                "UPDATE invoices SET draft = ?1 WHERE id = ?2",
                params![serde_json::to_string(&draft)?, id],
            )?;
        }
        tx.execute_batch("DROP TABLE document_links;")?;
    }
    tx.execute_batch("PRAGMA user_version = 15;")?;
    tx.commit()?;
    Ok(())
}

impl Store {
    /// The documents of the other kind a document is billed with, newest first: the offers an
    /// invoice bills, the invoices that bill an offer
    pub fn links(&self, id: i64) -> Result<Vec<Linked>> {
        let conn = self.conn();
        let others: std::collections::BTreeSet<i64> = references(&conn, None)?
            .into_iter()
            .filter_map(|r| match (r.invoice_id == id, r.offer_id == id) {
                (true, _) => Some(r.offer_id),
                (_, true) => Some(r.invoice_id),
                _ => None,
            })
            .collect();
        let mut linked = others
            .into_iter()
            .map(|other| linked(&conn, other))
            .collect::<Result<Vec<_>>>()?;
        linked.sort_by(|a, b| b.issue_date.cmp(&a.issue_date).then(b.id.cmp(&a.id)));
        Ok(linked)
    }

    /// The numbers of the created offers an invoice bills, by id
    pub fn offer_numbers(&self, invoice_id: i64) -> Result<Vec<String>> {
        let positions = self.invoice(invoice_id)?.draft.positions;
        numbers_of(&self.conn(), &positions)
    }

    /// The numbers of the created offers positions refer to, for a draft that is not stored yet
    pub fn offer_numbers_in(&self, positions: &[Position]) -> Result<Vec<String>> {
        numbers_of(&self.conn(), positions)
    }

    /// What is billed of a created offer, position by position; `except` leaves an invoice draft
    /// out (the one that asks, its own positions are on screen)
    pub fn billing(&self, offer_id: i64, except: Option<i64>) -> Result<OfferBilling> {
        let offer = self.invoice(offer_id)?;
        if offer.kind != Kind::Offer || offer.status != Status::Exported {
            bail!("only a created offer is billed");
        }
        let references: Vec<Reference> = references(&self.conn(), Some(offer_id))?
            .into_iter()
            .filter(|r| Some(r.invoice_id) != except)
            .collect();
        let sum = |index: usize, created: bool| -> Decimal {
            references
                .iter()
                .filter(|r| r.index == index && r.created == created)
                .map(|r| r.quantity)
                .sum()
        };
        let positions = offer
            .draft
            .positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| {
                let billed = sum(index, true);
                let open = (quantity(&position) - billed).max(Decimal::ZERO);
                PositionBilling {
                    index,
                    billed: text(billed),
                    drafted: text(sum(index, false)),
                    open: text(open),
                    position,
                }
            })
            .collect();
        let invoices = self
            .links(offer_id)?
            .into_iter()
            .filter(|l| l.kind == Kind::Invoice)
            .collect();
        Ok(OfferBilling {
            offer_id,
            number: offer.number.unwrap_or_default(),
            positions,
            invoices,
        })
    }

    /// What is billed of the offers of a Vorgang that can still be billed: created, neither
    /// declined, lapsed nor replaced; oldest first
    pub fn case_billing(&self, case_id: i64, except: Option<i64>) -> Result<Vec<OfferBilling>> {
        let ids: Vec<i64> = {
            let conn = self.conn();
            let mut statement = conn.prepare(
                "SELECT id FROM invoices
                 WHERE kind = 'offer' AND status = 'exported' AND (case_id = ?1 OR case_id IN (
                     SELECT source_id FROM case_links WHERE follower_id = ?1 AND kind = 'collect'
                 ))
                 ORDER BY id",
            )?;
            let ids = statement
                .query_map([case_id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        };
        ids.into_iter()
            .map(|id| self.invoice(id))
            .filter(|offer| {
                offer.as_ref().map_or(true, |o| {
                    o.answer != Some(Answer::Declined) && !o.lapsed && !o.superseded
                })
            })
            .map(|offer| self.billing(offer?.id, except))
            .collect()
    }

    /// A new invoice draft in the Vorgang of a created offer with everything still open of it,
    /// each position referring to the offer's: a Rechnung, or a Teilrechnung once less is billed.
    /// It has the customer, buyer reference and note of the offer; layout and texts are the
    /// invoice defaults of `settings` (an offer's intro does not belong on an invoice), dated
    /// `today`.
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
        // out of the running: the replacement or the accepted alternative is billed instead
        if offer.superseded || offer.lapsed {
            bail!("Ein ersetztes oder entfallenes Angebot wird nicht in Rechnung gestellt.");
        }
        let positions: Vec<Position> = self
            .billing(offer_id, None)?
            .positions
            .into_iter()
            .filter(|b| decimal(&b.open) > Decimal::ZERO)
            .map(|b| billed(b.position, &b.open, offer_id, b.index))
            .collect();
        if positions.is_empty() {
            bail!("Dieses Angebot ist vollständig in Rechnung gestellt.");
        }
        let mut draft = settings.blank_draft(Kind::Invoice, today);
        draft.customer = offer.draft.customer;
        draft.positions = positions;
        draft.buyer_reference = offer.draft.buyer_reference;
        draft.note = offer.draft.note;
        // the list shows what the draft comes to, as `invoice_save` keeps it
        let total = build_invoice(&draft, settings, "ENTWURF")
            .ok()
            .map(|invoice| totals(&invoice).due);
        let id = {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            let json = serde_json::to_string(&draft)?;
            let id = insert_draft(
                &tx,
                offer.context_id,
                Kind::Invoice,
                &json,
                total.as_deref(),
                Some(offer_id),
                offer.case_id,
            )?;
            tx.commit()?;
            id
        };
        self.invoice(id)
    }
}

/// An offer position as an invoice position billing `open` of it; a blank quantity stays blank
/// when all of it is billed
fn billed(mut position: Position, open: &str, offer_id: i64, index: usize) -> Position {
    if position.quantity.is_some() || decimal(open) != Decimal::ONE {
        position.quantity = Some(open.to_owned());
    }
    position.source = Some(PositionSource { offer_id, index });
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::tests::bake;
    use crate::context::Context;
    use crate::draft::fixtures::{draft, settings};

    fn store() -> Store {
        let store = Store::in_memory().unwrap();
        store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        store
    }

    fn position(description: &str, quantity: Option<&str>, price: &str) -> Position {
        Position {
            description: description.into(),
            quantity: quantity.map(str::to_owned),
            price: price.into(),
            ..Position::default()
        }
    }

    /// A created offer with two positions: 10 hours and a lump sum
    fn offer(store: &Store) -> StoredInvoice {
        let mut offer = draft();
        offer.positions = vec![
            position("Montage", Some("10"), "50"),
            position("Material", None, "200"),
        ];
        let saved = store
            .save_draft(None, 1, Kind::Offer, &offer, Some("700"))
            .unwrap();
        bake(store, saved.id, "AN-1", "700");
        store.invoice(saved.id).unwrap()
    }

    #[test]
    fn a_rechnung_bills_all_that_is_open_and_a_second_what_is_left() {
        let store = store();
        let offer = offer(&store);
        let first = store
            .invoice_from_offer(offer.id, "2026-11-02", &settings())
            .unwrap();
        assert_eq!(first.case_id, offer.case_id);
        assert_eq!(
            first
                .draft
                .positions
                .iter()
                .map(|p| (p.quantity.clone(), p.source))
                .collect::<Vec<_>>(),
            [
                (
                    Some("10".into()),
                    Some(PositionSource {
                        offer_id: offer.id,
                        index: 0
                    })
                ),
                (
                    None,
                    Some(PositionSource {
                        offer_id: offer.id,
                        index: 1
                    })
                ),
            ]
        );
        // a draft does not bill yet, it shows apart
        let billing = store.billing(offer.id, None).unwrap();
        assert_eq!(billing.positions[0].open, "10");
        assert_eq!(billing.positions[0].drafted, "10");
        assert_eq!(billing.invoices.len(), 1);

        // a Teilrechnung: 4 of the hours, no material
        let mut part = first.draft.clone();
        part.positions[0].quantity = Some("4".into());
        part.positions.pop();
        store
            .save_draft(Some(first.id), 1, Kind::Invoice, &part, None)
            .unwrap();
        bake(&store, first.id, "RE-1", "238");
        let billing = store.billing(offer.id, None).unwrap();
        assert_eq!(
            billing
                .positions
                .iter()
                .map(|b| (b.billed.as_str(), b.open.as_str()))
                .collect::<Vec<_>>(),
            [("4", "6"), ("0", "1")]
        );

        // the next one bills the rest
        let second = store
            .invoice_from_offer(offer.id, "2026-11-03", &settings())
            .unwrap();
        assert_eq!(second.draft.positions[0].quantity.as_deref(), Some("6"));
        assert_eq!(second.draft.positions.len(), 2);
        bake(&store, second.id, "RE-2", "500");
        assert!(store
            .invoice_from_offer(offer.id, "2026-11-04", &settings())
            .is_err());

        // what bills what, both ways
        assert_eq!(store.offer_numbers(second.id).unwrap(), ["AN-1"]);
        let invoices: Vec<_> = store
            .links(offer.id)
            .unwrap()
            .into_iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(invoices, [second.id, first.id]);
        assert_eq!(store.links(first.id).unwrap()[0].id, offer.id);
        let counts = reference_counts(&store.conn()).unwrap();
        assert_eq!((counts[&offer.id], counts[&first.id]), (2, 1));
        let listed = store.invoices(None, None, false).unwrap();
        assert_eq!(listed.iter().find(|e| e.id == offer.id).unwrap().linked, 2);
    }

    #[test]
    fn an_invoice_says_which_offers_it_bills() {
        let store = store();
        let offer = offer(&store);
        let invoice = store
            .invoice_from_offer(offer.id, "2026-11-02", &settings())
            .unwrap();
        let (_, branding) = crate::commands::preview_invoice_command::shown(
            &store,
            &invoice.draft,
            Some(invoice.id),
            1,
            Kind::Invoice,
        )
        .unwrap()
        .unwrap();
        assert_eq!(branding.offer_number, "AN-1");
        // a saved position bills nothing
        let saved = store
            .save_template(&crate::model::PositionTemplate {
                id: None,
                context_id: Some(1),
                position: invoice.draft.positions[0].clone(),
            })
            .unwrap();
        assert_eq!(saved.position.source, None);
    }

    #[test]
    fn quantities_count_as_they_are_typed() {
        assert_eq!(
            quantity(&position("x", Some("2,5"), "1")),
            Decimal::new(25, 1)
        );
        assert_eq!(
            quantity(&position("x", Some("1.000,5"), "1")),
            Decimal::new(10005, 1)
        );
        assert_eq!(quantity(&position("x", Some(" "), "1")), Decimal::ONE);
        assert_eq!(quantity(&position("x", None, "1")), Decimal::ONE);
    }

    #[test]
    fn a_draft_asking_leaves_itself_out() {
        let store = store();
        let offer = offer(&store);
        let invoice = store
            .invoice_from_offer(offer.id, "2026-11-02", &settings())
            .unwrap();
        let billing = store.billing(offer.id, Some(invoice.id)).unwrap();
        assert_eq!(billing.positions[0].drafted, "0");
        assert_eq!(store.case_billing(offer.case_id, None).unwrap().len(), 1);
    }

    #[test]
    fn positions_refer_only_to_created_offers_of_their_vorgang() {
        let store = store();
        let offer = offer(&store);
        let invoice = store
            .invoice_from_offer(offer.id, "2026-11-02", &settings())
            .unwrap();
        let refer = |offer_id: i64, index: usize| {
            let mut changed = invoice.draft.clone();
            changed.positions[0].source = Some(PositionSource { offer_id, index });
            store.save_draft(Some(invoice.id), 1, Kind::Invoice, &changed, None)
        };
        assert!(refer(offer.id, 1).is_ok());
        assert!(refer(offer.id, 2).is_err());
        assert!(refer(999, 0).is_err());
        // an offer of another Vorgang
        let other = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap();
        bake(&store, other.id, "AN-2", "1");
        assert!(refer(other.id, 0).is_err());
        // an offer's positions refer to nothing
        let mut sourced = draft();
        sourced.positions[0].source = Some(PositionSource {
            offer_id: offer.id,
            index: 0,
        });
        assert!(store
            .save_draft(None, 1, Kind::Offer, &sourced, None)
            .is_err());
        // a copy bills nothing again
        let copy = store.duplicate_to(invoice.id, "2026-11-03", true).unwrap();
        assert!(copy.draft.positions.iter().all(|p| p.source.is_none()));
    }

    #[test]
    fn links_of_older_versions_become_references() {
        let store = store();
        let offer = offer(&store);
        let mut invoice = draft();
        invoice.positions = vec![
            position("Material", None, "200"),
            position("Montage", Some("3"), "50"),
            position("Anfahrt", None, "30"),
        ];
        let invoice = store
            .save_draft_in_case(offer.case_id, Kind::Invoice, &invoice, None)
            .unwrap();
        {
            let conn = store.conn();
            conn.execute_batch(
                "CREATE TABLE document_links (offer_id INTEGER, invoice_id INTEGER, created_at TEXT);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO document_links VALUES (?1, ?2, '2026-10-01')",
                [offer.id, invoice.id],
            )
            .unwrap();
            migrate_to_sources(&conn).unwrap();
        }
        let sources: Vec<_> = store
            .invoice(invoice.id)
            .unwrap()
            .draft
            .positions
            .iter()
            .map(|p| p.source.map(|s| s.index))
            .collect();
        assert_eq!(sources, [Some(1), Some(0), None]);
        assert_eq!(store.offer_numbers(invoice.id).unwrap(), ["AN-1"]);
    }
}
