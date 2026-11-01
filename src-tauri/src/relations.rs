//! Offers that refer to another offer of their Vorgang: a Nachtrag (more work in the same case), an
//! Alternative (a variant, one offer of the group is taken) and an Ersatzangebot (replaces it). The
//! offer referred to is a created one, so its number is fixed; the relation is set when the new
//! offer is made and stays for good, like its kind.
//!
//! What follows from the relations is read from the data, never stored: an offer of an alternative
//! group lapses (`lapsed`) once another offer of the group is accepted, an offer is superseded
//! (`superseded`) once an Ersatzangebot for it is created.

use anyhow::{anyhow, bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::model::{Answer, DocumentRef, Kind, Status, StoredInvoice};
use crate::store::{insert_draft, Store};

/// How an offer refers to the created offer it was made for
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    /// Nachtrag: more work in the same case, refers to the main offer
    Supplement,
    /// Alternativangebot: a variant, of a group only one is accepted, the others lapse
    Alternative,
    /// Ersatzangebot: replaces the offer, which is superseded once it is created
    Replacement,
}

impl Relation {
    /// As the `relation` column spells it
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supplement => "supplement",
            Self::Alternative => "alternative",
            Self::Replacement => "replacement",
        }
    }

    pub fn from_column(text: &str) -> Option<Self> {
        match text {
            "supplement" => Some(Self::Supplement),
            "alternative" => Some(Self::Alternative),
            "replacement" => Some(Self::Replacement),
            _ => None,
        }
    }

    /// The label of the row on the document that names the offer referred to, by what offers are
    /// called (`title`, "Angebot")
    pub fn label(self, title: &str) -> String {
        match self {
            Self::Supplement => format!("Nachtrag zu {title}"),
            Self::Alternative => format!("Alternative zu {title}"),
            Self::Replacement => format!("Ersetzt {title}"),
        }
    }
}

/// The relation and the offer it refers to, on the documents table
pub(crate) fn migrate_to_relations(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    if !crate::store::has_column(&tx, "invoices", "relation")? {
        tx.execute_batch(
            "ALTER TABLE invoices ADD COLUMN relation TEXT;
             ALTER TABLE invoices ADD COLUMN related_id INTEGER;",
        )?;
    }
    tx.execute_batch(
        "CREATE INDEX IF NOT EXISTS invoices_related ON invoices (related_id);
         PRAGMA user_version = 14;",
    )?;
    tx.commit()?;
    Ok(())
}

/// The columns a read of `invoices` adds for the relations (and the Stornorechnung that cancels an
/// invoice, created or drafted, see `corrections`): the number of the offer referred to, the created
/// Ersatzangebot superseding the row and the other offer of its group that is accepted (`taken_by`;
/// an unanswered offer of a taken group lapsed, see `lapsed`). Documents named come as JSON of
/// their id and number (see `reference`). An
/// alternative's group is the offer it refers to (`related_id`), every other offer is the head of
/// its own.
pub(crate) const RELATION_COLUMNS: &str = "
    relation, related_id,
    (SELECT r.number FROM invoices r WHERE r.id = invoices.related_id) AS related_number,
    (SELECT json_object('id', r.id, 'number', r.number) FROM invoices r
        WHERE r.related_id = invoices.id AND r.relation = 'replacement' AND r.status = 'exported'
        LIMIT 1
    ) AS superseded_by,
    (SELECT json_object('id', m.id, 'number', m.number) FROM invoices m
        WHERE invoices.kind = 'offer' AND m.id <> invoices.id AND m.answer = 'accepted' AND (
            m.id = (CASE WHEN invoices.relation = 'alternative'
                         THEN invoices.related_id ELSE invoices.id END)
            OR (m.relation = 'alternative' AND m.related_id = (CASE
                WHEN invoices.relation = 'alternative' THEN invoices.related_id
                ELSE invoices.id END))
        )
        LIMIT 1
    ) AS taken_by,
    (SELECT json_object('id', s.id, 'number', s.number) FROM invoices s
        WHERE s.status = 'exported' AND json_extract(s.draft, '$.corrects.kind') = 'cancel'
            AND json_extract(s.draft, '$.corrects.invoice_id') = invoices.id
        LIMIT 1
    ) AS cancelled_by,
    (SELECT s.id FROM invoices s
        WHERE s.status = 'draft' AND json_extract(s.draft, '$.corrects.kind') = 'cancel'
            AND json_extract(s.draft, '$.corrects.invoice_id') = invoices.id
        LIMIT 1
    ) AS cancel_draft";

/// A document a row's column names, as `RELATION_COLUMNS` reads it
pub(crate) fn reference(
    row: &rusqlite::Row,
    column: &str,
) -> rusqlite::Result<Option<DocumentRef>> {
    Ok(row
        .get::<_, Option<String>>(column)?
        .and_then(|json| serde_json::from_str(&json).ok()))
}

/// A created offer nobody answered whose group has another offer accepted is out of the running
pub(crate) fn lapsed(kind: Kind, status: Status, answer: Option<Answer>, taken: bool) -> bool {
    kind == Kind::Offer && status == Status::Exported && answer.is_none() && taken
}

/// Whether the offer `id` is superseded, and whether another offer of its group is accepted
fn standing(conn: &Connection, id: i64) -> Result<(bool, bool)> {
    Ok(conn.query_row(
        &format!("SELECT {RELATION_COLUMNS} FROM invoices WHERE id = ?1"),
        [id],
        |r| {
            Ok((
                reference(r, "superseded_by")?.is_some(),
                reference(r, "taken_by")?.is_some(),
            ))
        },
    )?)
}

impl Store {
    /// A new offer draft in the Vorgang of the created offer `source` that refers to it as
    /// `relation`, dated `today`. A Nachtrag starts blank with the offer defaults of the settings
    /// and the customer of the source, an alternative and a replacement as a copy of the source.
    /// A Nachtrag to a Nachtrag and an alternative to an alternative refer to what theirs refers
    /// to, so a main offer has its Nachträge and a group its alternatives side by side.
    pub fn related_offer(
        &self,
        source: i64,
        relation: Relation,
        today: &str,
    ) -> Result<StoredInvoice> {
        let offer = self.invoice(source)?;
        if offer.kind != Kind::Offer || offer.status != Status::Exported {
            bail!("Nachtrag, Alternative und Ersatz gibt es nur zu einem erstellten Angebot.");
        }
        let target = match (relation, offer.relation, offer.related_id) {
            (Relation::Supplement, Some(Relation::Supplement), Some(main))
            | (Relation::Alternative, Some(Relation::Alternative), Some(main)) => main,
            _ => source,
        };
        match relation {
            Relation::Supplement if offer.answer == Some(Answer::Declined) => {
                bail!("Ein abgelehntes Angebot bekommt keinen Nachtrag.")
            }
            Relation::Alternative
                if offer.answer == Some(Answer::Accepted) || standing(&self.conn(), source)?.1 =>
            {
                bail!("Ein Angebot dieser Gruppe ist schon angenommen, eine Alternative kommt zu spät.")
            }
            Relation::Replacement if offer.answer == Some(Answer::Accepted) => {
                bail!("Ein angenommenes Angebot wird nicht ersetzt.")
            }
            _ => {}
        }
        let mut draft = match relation {
            Relation::Supplement => {
                let mut blank = self
                    .settings(offer.context_id)?
                    .blank_draft(Kind::Offer, today);
                blank.customer = offer.draft.customer.clone();
                blank
                    .buyer_reference
                    .clone_from(&offer.draft.buyer_reference);
                blank
            }
            Relation::Alternative | Relation::Replacement => offer.draft.clone(),
        };
        draft.issue_date = today.into();
        draft.delivery_date = today.into();
        draft.delivery_end = None;
        draft.frozen = None;
        draft.frozen_layout = None;
        draft.frozen_offers = Vec::new();
        let json = serde_json::to_string(&draft)?;
        let id = {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            let id = insert_draft(
                &tx,
                offer.context_id,
                Kind::Offer,
                &json,
                None,
                Some(source),
                offer.case_id,
            )?;
            tx.execute(
                "UPDATE invoices SET relation = ?1, related_id = ?2 WHERE id = ?3",
                params![relation.as_str(), target, id],
            )?;
            tx.commit()?;
            id
        };
        self.invoice(id)
    }

    /// How a document refers to another offer and that offer's number, for the row that names it
    pub fn relation_of(&self, id: i64) -> Result<Option<(Relation, String)>> {
        let row: Option<(Option<String>, Option<String>)> = self
            .conn()
            .query_row(
                "SELECT i.relation, r.number FROM invoices i
                 LEFT JOIN invoices r ON r.id = i.related_id WHERE i.id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (relation, number) = row.ok_or_else(|| anyhow!("unknown invoice"))?;
        Ok(relation
            .as_deref()
            .and_then(Relation::from_column)
            .zip(number))
    }

    /// Accepting an offer of a group whose other offer is accepted, or a superseded one, is refused
    pub(crate) fn may_accept(&self, id: i64) -> Result<()> {
        let (superseded, taken) = standing(&self.conn(), id)?;
        if superseded {
            bail!("Ein ersetztes Angebot wird nicht angenommen, sein Ersatzangebot schon.");
        }
        if taken {
            bail!("Ein anderes Angebot dieser Gruppe ist schon angenommen.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::tests::bake;
    use crate::context::Context;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::CaseStage;

    /// A store with the context 1, whose settings a Nachtrag starts from
    fn store() -> Store {
        let store = Store::in_memory().unwrap();
        let context = store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        assert_eq!(context.id, Some(1));
        store
    }

    fn created_offer(store: &Store, number: &str, total: &str) -> StoredInvoice {
        let offer = store
            .save_draft(None, 1, Kind::Offer, &draft(), Some(total))
            .unwrap();
        bake(store, offer.id, number, total);
        store.invoice(offer.id).unwrap()
    }

    fn related(store: &Store, source: i64, relation: Relation) -> StoredInvoice {
        store.related_offer(source, relation, "2026-11-02").unwrap()
    }

    #[test]
    fn a_related_offer_is_made_in_the_vorgang_of_a_created_one() {
        let store = store();
        let draft_offer = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap();
        assert!(store
            .related_offer(draft_offer.id, Relation::Supplement, "2026-11-02")
            .is_err());

        let main = created_offer(&store, "AN-1", "100");
        let supplement = related(&store, main.id, Relation::Supplement);
        assert_eq!(supplement.case_id, main.case_id);
        assert_eq!(supplement.relation, Some(Relation::Supplement));
        assert_eq!(supplement.related_id, Some(main.id));
        assert_eq!(supplement.related_number.as_deref(), Some("AN-1"));
        assert_eq!(supplement.draft.issue_date, "2026-11-02");
        assert_eq!(supplement.draft.customer, main.draft.customer);
        // a Nachtrag is more work, it starts without the positions of the main offer
        assert_ne!(supplement.draft.positions, main.draft.positions);

        let alternative = related(&store, main.id, Relation::Alternative);
        assert_eq!(alternative.draft.positions, main.draft.positions);
        assert_eq!(alternative.draft.frozen, None);
        assert_eq!(
            store.relation_of(alternative.id).unwrap(),
            Some((Relation::Alternative, "AN-1".into()))
        );
        assert_eq!(store.relation_of(main.id).unwrap(), None);
        // it says what it was made from
        let events = store.events(alternative.id).unwrap();
        assert_eq!(events[0].detail.from, Some(main.id));
    }

    #[test]
    fn the_document_names_the_offer_it_refers_to() {
        let store = store();
        let main = created_offer(&store, "AN-1", "100");
        let supplement = related(&store, main.id, Relation::Supplement);
        let shown = |id: i64| {
            let stored = store.invoice(id).unwrap();
            crate::commands::preview_invoice_command::shown(
                &store,
                &stored.draft,
                Some(id),
                1,
                Kind::Offer,
            )
            .unwrap()
            .unwrap()
            .1
        };
        let branding = shown(supplement.id);
        assert_eq!(branding.offers, ["AN-1"]);
        assert_eq!(branding.offer_label, "Nachtrag zu Angebot");
        assert!(shown(main.id).offers.is_empty());
    }

    #[test]
    fn a_nachtrag_to_a_nachtrag_and_an_alternative_to_an_alternative_refer_to_the_head() {
        let store = store();
        let main = created_offer(&store, "AN-1", "100");
        let first = related(&store, main.id, Relation::Supplement);
        bake(&store, first.id, "AN-2", "20");
        assert_eq!(
            related(&store, first.id, Relation::Supplement).related_id,
            Some(main.id)
        );
        let alternative = related(&store, main.id, Relation::Alternative);
        bake(&store, alternative.id, "AN-3", "90");
        assert_eq!(
            related(&store, alternative.id, Relation::Alternative).related_id,
            Some(main.id)
        );
        // a replacement replaces the offer it was made for
        assert_eq!(
            related(&store, first.id, Relation::Replacement).related_id,
            Some(first.id)
        );
    }

    #[test]
    fn the_others_of_a_group_lapse_when_one_is_accepted() {
        let store = store();
        let main = created_offer(&store, "AN-1", "100");
        let alternative = related(&store, main.id, Relation::Alternative);
        bake(&store, alternative.id, "AN-2", "80");
        let supplement = related(&store, main.id, Relation::Supplement);
        bake(&store, supplement.id, "AN-3", "10");
        let lapsed = |id: i64| store.invoice(id).unwrap().lapsed;
        assert!(!lapsed(main.id) && !lapsed(alternative.id));

        store
            .set_answer(alternative.id, Some((Answer::Accepted, "2026-11-03")))
            .unwrap();
        assert!(lapsed(main.id));
        assert!(!lapsed(alternative.id));
        // it names the one accepted instead
        assert_eq!(
            store.invoice(main.id).unwrap().taken_by,
            Some(DocumentRef {
                id: alternative.id,
                number: Some("AN-2".into())
            })
        );
        // a Nachtrag is no alternative, it stays
        assert!(!lapsed(supplement.id));
        let listed = store.invoices(None, Some(Kind::Offer), false).unwrap();
        assert!(listed.iter().find(|e| e.id == main.id).unwrap().lapsed);
        // nor billed
        assert!(store
            .invoice_from_offer(main.id, "2026-11-03", &settings())
            .is_err());
        // the main offer cannot be accepted too, nor get another alternative
        assert!(store
            .set_answer(main.id, Some((Answer::Accepted, "2026-11-03")))
            .is_err());
        assert!(store
            .related_offer(main.id, Relation::Alternative, "2026-11-03")
            .is_err());
        // declining it is fine, accepting it after that still is not
        store
            .set_answer(main.id, Some((Answer::Declined, "2026-11-03")))
            .unwrap();
        assert!(store
            .set_answer(main.id, Some((Answer::Accepted, "2026-11-03")))
            .is_err());
        assert!(!lapsed(main.id));
        // declined, it is not billed either
        assert!(store
            .invoice_from_offer(main.id, "2026-11-03", &settings())
            .is_err());

        // the Vorgang counts what was accepted, the lapsed offer is not offered any more
        let case = store.case(main.case_id).unwrap();
        assert_eq!(case.stage, CaseStage::Accepted);
        assert_eq!(case.offered, "90");
        assert_eq!(case.accepted, "80");
    }

    #[test]
    fn an_offer_is_superseded_once_its_replacement_is_created() {
        let store = store();
        let old = created_offer(&store, "AN-1", "100");
        let replacement = related(&store, old.id, Relation::Replacement);
        assert!(!store.invoice(old.id).unwrap().superseded);
        bake(&store, replacement.id, "AN-2", "120");
        let superseded = store.invoice(old.id).unwrap();
        assert!(superseded.superseded);
        assert_eq!(superseded.superseded_by.unwrap().id, replacement.id);
        assert!(store
            .set_answer(old.id, Some((Answer::Accepted, "2026-11-03")))
            .is_err());
        // only the replacement is on offer
        assert_eq!(store.case(old.case_id).unwrap().offered, "120");

        store
            .set_answer(replacement.id, Some((Answer::Accepted, "2026-11-03")))
            .unwrap();
        assert!(store
            .related_offer(replacement.id, Relation::Replacement, "2026-11-03")
            .is_err());
    }

    #[test]
    fn unanswered_alternatives_are_offered_once() {
        let store = store();
        let main = created_offer(&store, "AN-1", "100");
        let alternative = related(&store, main.id, Relation::Alternative);
        bake(&store, alternative.id, "AN-2", "80");
        let supplement = related(&store, main.id, Relation::Supplement);
        bake(&store, supplement.id, "AN-3", "10");
        // the main offer and its Nachtrag, not the variant on top
        assert_eq!(store.case(main.case_id).unwrap().offered, "110");
    }

    #[test]
    fn a_declined_offer_gets_no_nachtrag() {
        let store = store();
        let main = created_offer(&store, "AN-1", "100");
        store
            .set_answer(main.id, Some((Answer::Declined, "2026-11-03")))
            .unwrap();
        assert!(store
            .related_offer(main.id, Relation::Supplement, "2026-11-03")
            .is_err());
        // a better offer instead is fine
        related(&store, main.id, Relation::Replacement);
    }
}
