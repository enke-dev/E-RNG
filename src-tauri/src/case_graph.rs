//! The documents of a Vorgang and how they relate, for the graph on the Vorgang's page. Nothing is
//! stored for it: the edges are read from what the documents say already. An offer refers to the
//! offer it is a Nachtrag, an alternative or a replacement for (`relation`); an invoice bills offer
//! positions (`Position.source`), takes a share of an offer (`Position.share`), deducts invoices on
//! account (`deductions`) and cancels or replaces an invoice (`corrects`). Documents of other
//! Vorgänge on either end (a Sammelrechnung and the offers it bills) come along, the graph shows
//! them as belonging elsewhere.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use crate::model::{CorrectionKind, InvoiceDraft, ListEntry};
use crate::relations::Relation;
use crate::store::Store;

/// How one document relates to another, the edge running from the earlier to the later
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// An invoice bills positions of an offer
    Bills,
    /// An invoice on account takes a share of an offer
    Share,
    /// A Schlussrechnung deducts an invoice on account
    Deducts,
    Supplement,
    Alternative,
    Replacement,
    /// A Stornorechnung cancels an invoice
    Cancels,
    /// An Ersatzrechnung replaces a cancelled invoice
    Replaces,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct GraphEdge {
    pub from: i64,
    pub to: i64,
    pub kind: EdgeKind,
}

/// Another Vorgang a document of the graph is in, by its number
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OtherCase {
    pub id: i64,
    pub number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseGraph {
    /// Its documents and those of other Vorgänge they relate to, by issue date
    pub documents: Vec<ListEntry>,
    pub edges: Vec<GraphEdge>,
    /// The other Vorgänge among the documents
    pub others: Vec<OtherCase>,
}

/// The edges a document's own data names, ending at it
fn edges_of(
    id: i64,
    relation: Option<Relation>,
    related: Option<i64>,
    draft: &InvoiceDraft,
) -> Vec<GraphEdge> {
    let edge = |from: i64, kind: EdgeKind| GraphEdge { from, to: id, kind };
    let relation = relation.zip(related).map(|(relation, from)| {
        edge(
            from,
            match relation {
                Relation::Supplement => EdgeKind::Supplement,
                Relation::Alternative => EdgeKind::Alternative,
                Relation::Replacement => EdgeKind::Replacement,
            },
        )
    });
    let bills = draft
        .positions
        .iter()
        .filter_map(|p| p.source.map(|s| edge(s.offer_id, EdgeKind::Bills)));
    let shares = draft
        .positions
        .iter()
        .filter_map(|p| p.share.as_ref().map(|s| edge(s.offer_id, EdgeKind::Share)));
    let deducts = draft
        .deductions
        .iter()
        .map(|d| edge(d.invoice_id, EdgeKind::Deducts));
    let corrects = draft.corrects.as_ref().map(|c| {
        edge(
            c.invoice_id,
            match c.kind {
                CorrectionKind::Cancel => EdgeKind::Cancels,
                CorrectionKind::Replace => EdgeKind::Replaces,
            },
        )
    });
    relation
        .into_iter()
        .chain(bills)
        .chain(shares)
        .chain(deducts)
        .chain(corrects)
        .collect()
}

impl Store {
    /// The documents of the Vorgang `id`, archived ones too, how they relate, and the documents of
    /// other Vorgänge on the other end
    pub fn case_graph(&self, id: i64) -> Result<CaseGraph> {
        let (own, incoming) = {
            let conn = self.conn();
            let own = conn
                .prepare("SELECT id, relation, related_id, draft FROM invoices WHERE case_id = ?1")?
                .query_map([id], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            // invoices of other Vorgänge billing its offers: a Sammelrechnung collecting it
            let incoming = conn
                .prepare(
                    "SELECT DISTINCT i.id, json_extract(p.value, '$.source.offer_id')
                     FROM invoices i, json_each(i.draft, '$.positions') p
                     WHERE i.case_id <> ?1 AND json_extract(p.value, '$.source.offer_id') IN
                         (SELECT id FROM invoices WHERE case_id = ?1 AND kind = 'offer')",
                )?
                .query_map(params![id], |r| {
                    Ok(GraphEdge {
                        from: r.get(1)?,
                        to: r.get(0)?,
                        kind: EdgeKind::Bills,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (own, incoming)
        };
        let own_edges = own
            .iter()
            .map(|(doc, relation, related, draft)| {
                let draft: InvoiceDraft = serde_json::from_str(draft)?;
                let relation = relation.as_deref().and_then(Relation::from_column);
                Ok(edges_of(*doc, relation, *related, &draft))
            })
            .collect::<Result<Vec<_>>>()?;
        // one edge per pair and kind, however many positions say it
        let edges: Vec<GraphEdge> = own_edges
            .into_iter()
            .flatten()
            .chain(incoming)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ids: BTreeSet<i64> = own
            .iter()
            .map(|(doc, ..)| *doc)
            .chain(edges.iter().flat_map(|e| [e.from, e.to]))
            .collect();
        let mut documents: Vec<ListEntry> = self
            .invoices(None, None, true)?
            .into_iter()
            .filter(|d| ids.contains(&d.id))
            .collect();
        documents.sort_by(|a, b| a.issue_date.cmp(&b.issue_date).then(a.id.cmp(&b.id)));
        // an edge to a document that is gone (a deleted draft) is not drawn
        let present: BTreeSet<i64> = documents.iter().map(|d| d.id).collect();
        let edges = edges
            .into_iter()
            .filter(|e| present.contains(&e.from) && present.contains(&e.to))
            .collect();
        let others: BTreeMap<i64, String> = documents
            .iter()
            .filter(|d| d.case_id != id)
            .map(|d| Ok((d.case_id, self.case(d.case_id)?.number)))
            .collect::<Result<_>>()?;
        Ok(CaseGraph {
            documents,
            edges,
            others: others
                .into_iter()
                .map(|(id, number)| OtherCase { id, number })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::tests::bake;
    use crate::context::Context;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::Kind;

    fn store() -> Store {
        let store = Store::in_memory().unwrap();
        store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        store
    }

    #[test]
    fn a_vorgang_draws_what_its_documents_say() {
        let store = store();
        let mut written = draft();
        written.customer.id = Some(7);
        let offer = store
            .save_draft(None, 1, Kind::Offer, &written, None)
            .unwrap();
        bake(&store, offer.id, "AN-1", "100");
        let supplement = store
            .related_offer(offer.id, Relation::Supplement, "2026-11-03")
            .unwrap();
        let invoice = store
            .invoice_from_offer(offer.id, "2026-11-03", &settings())
            .unwrap();
        bake(&store, invoice.id, "RE-1", "238");
        let storno = store.cancel_invoice(invoice.id, "2026-11-04").unwrap();

        let graph = store.case_graph(offer.case_id).unwrap();
        assert_eq!(graph.documents.len(), 4);
        assert!(graph.others.is_empty());
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|e| (e.from, e.to, e.kind))
                .collect::<Vec<_>>(),
            [
                (offer.id, supplement.id, EdgeKind::Supplement),
                (offer.id, invoice.id, EdgeKind::Bills),
                (invoice.id, storno.id, EdgeKind::Cancels),
            ]
        );
    }

    #[test]
    fn a_collected_vorgang_shows_the_invoice_collecting_it() {
        let store = store();
        let offer = |number: &str| {
            let mut written = draft();
            written.customer.id = Some(7);
            let saved = store
                .save_draft(None, 1, Kind::Offer, &written, None)
                .unwrap();
            bake(&store, saved.id, number, "100");
            saved
        };
        let (a, b) = (offer("AN-1"), offer("AN-2"));
        let invoice = store
            .collect(&[a.case_id, b.case_id], "2026-11-03", "Sammel")
            .unwrap();
        let graph = store.case_graph(a.case_id).unwrap();
        assert_eq!(
            graph.documents.iter().map(|d| d.id).collect::<Vec<_>>(),
            [a.id, invoice.id]
        );
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.others[0].id, invoice.case_id);
        // and the collecting one shows the offers of both
        let collecting = store.case_graph(invoice.case_id).unwrap();
        assert_eq!(collecting.edges.len(), 2);
        assert_eq!(collecting.others.len(), 2);
    }

    #[test]
    fn an_invoice_on_account_draws_a_line_from_each_offer_it_shares() {
        use crate::model::{OnAccount, PositionShare};
        let store = store();
        let mut written = draft();
        written.customer.id = Some(7);
        let a = store
            .save_draft(None, 1, Kind::Offer, &written, None)
            .unwrap();
        bake(&store, a.id, "AN-1", "100");
        let b = store
            .save_draft_in_case(a.case_id, Kind::Offer, &written, None)
            .unwrap();
        bake(&store, b.id, "AN-2", "100");
        let mut progress = draft();
        progress.customer.id = Some(7);
        progress.on_account = Some(OnAccount::Progress);
        progress.positions = [a.id, b.id]
            .iter()
            .map(|offer| {
                let mut p = progress.positions[0].clone();
                p.share = Some(PositionShare {
                    offer_id: *offer,
                    percent: "30".into(),
                });
                p
            })
            .collect();
        let invoice = store
            .save_draft_in_case(a.case_id, Kind::Invoice, &progress, None)
            .unwrap();
        bake(&store, invoice.id, "RE-1", "71.40");
        let graph = store.case_graph(a.case_id).unwrap();
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|e| (e.from, e.to, e.kind))
                .collect::<Vec<_>>(),
            [
                (a.id, invoice.id, EdgeKind::Share),
                (b.id, invoice.id, EdgeKind::Share),
            ]
        );
    }
}
