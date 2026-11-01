//! Correcting a created invoice, which never changes itself (GoBD): a Stornorechnung cancels it, a
//! corrected invoice (384) that mirrors it with negative quantities (`CorrectionKind::Cancel`), and
//! an Ersatzrechnung replaces the cancelled invoice with a new one (`CorrectionKind::Replace`).
//! Both are drafts first, in the Vorgang of the invoice, and name it as a preceding invoice. A
//! Rechnungskorrektur is the two of them. A credit note (381) is not used: its amounts must be
//! positive, a Storno is expected to read negative.
//!
//! An invoice is cancelled once its Stornorechnung is created (`StoredInvoice.cancelled`): it is no
//! longer open, bills no offer position and is deducted by no Schlussrechnung. The Storno's totals
//! are negative, so the sums of the Vorgang come out as if neither was there.

use anyhow::{anyhow, bail, Result};
use rusqlite::{Connection, OptionalExtension};

use crate::model::{
    Correction, CorrectionKind, InvoiceDraft, Kind, Position, Status, StoredInvoice,
};
use crate::store::{insert_draft, Store};

/// The original of a correction: kind, status, Vorgang and data
fn original(
    conn: &Connection,
    id: i64,
) -> Result<(Kind, Status, i64, Option<String>, InvoiceDraft)> {
    let row: Option<(String, String, i64, Option<String>, String)> = conn
        .query_row(
            "SELECT kind, status, case_id, number, draft FROM invoices WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let (kind, status, case, number, draft) = row.ok_or_else(|| anyhow!("unknown invoice"))?;
    let status = if status == "exported" {
        Status::Exported
    } else {
        Status::Draft
    };
    Ok((
        Kind::from_column(&kind),
        status,
        case,
        number,
        serde_json::from_str(&draft)?,
    ))
}

/// Whether a created Stornorechnung other than `except` cancels the invoice `id`
fn cancelled_by_other(conn: &Connection, id: i64, except: Option<i64>) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT COUNT(*) > 0 FROM invoices
         WHERE status = 'exported' AND id IS NOT ?2
             AND json_extract(draft, '$.corrects.kind') = 'cancel'
             AND json_extract(draft, '$.corrects.invoice_id') = ?1",
        rusqlite::params![id, except],
        |r| r.get(0),
    )?)
}

/// A typed quantity taken back, a blank one being 1
fn negated(quantity: Option<&str>) -> String {
    let typed = quantity
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .unwrap_or("1");
    typed
        .strip_prefix('-')
        .map_or_else(|| format!("-{typed}"), str::to_owned)
}

/// The positions a Stornorechnung mirrors: the original's taken back (unit prices stay positive,
/// BR-27), billing nothing
fn mirrored(positions: &[Position]) -> Vec<Position> {
    positions
        .iter()
        .map(|p| Position {
            source: None,
            quantity: Some(negated(p.quantity.as_deref())),
            ..p.clone()
        })
        .collect()
}

/// A correction must name a created invoice of its Vorgang. A Stornorechnung mirrors it (customer,
/// positions and deductions as they were) and is the only one for it; an Ersatzrechnung replaces a cancelled
/// invoice once it is created (`creating`).
pub(crate) fn check_correction(
    conn: &Connection,
    case_id: i64,
    id: Option<i64>,
    kind: Kind,
    draft: &InvoiceDraft,
    creating: bool,
) -> Result<()> {
    let Some(correction) = &draft.corrects else {
        return Ok(());
    };
    if kind != Kind::Invoice {
        bail!("Nur eine Rechnung wird storniert oder ersetzt.");
    }
    let (original_kind, status, case, _, data) = original(conn, correction.invoice_id)?;
    if original_kind != Kind::Invoice || status != Status::Exported || case != case_id {
        bail!("Storniert oder ersetzt wird eine erstellte Rechnung desselben Vorgangs.");
    }
    if data
        .corrects
        .as_ref()
        .is_some_and(|c| c.kind == CorrectionKind::Cancel)
    {
        bail!("Eine Stornorechnung wird nicht storniert.");
    }
    match correction.kind {
        CorrectionKind::Cancel => {
            if cancelled_by_other(conn, correction.invoice_id, id)? {
                bail!("Diese Rechnung ist schon storniert.");
            }
            if draft.customer != data.customer
                || draft.positions != mirrored(&data.positions)
                || draft.deductions != data.deductions
                || (&draft.delivery_date, &draft.delivery_end)
                    != (&data.delivery_date, &data.delivery_end)
            {
                bail!("Eine Stornorechnung gibt die Rechnung so wieder, wie sie war.");
            }
        }
        CorrectionKind::Replace => {
            if creating && !cancelled_by_other(conn, correction.invoice_id, id)? {
                bail!("Eine Ersatzrechnung ersetzt eine stornierte Rechnung; erst die Stornorechnung erstellen.");
            }
        }
    }
    Ok(())
}

impl Store {
    /// Fills in the number and date of the invoice a draft corrects, from the invoice itself
    pub fn fill_correction(&self, draft: &mut InvoiceDraft) {
        if let Some(correction) = &mut draft.corrects {
            if let Ok((_, _, _, number, data)) = original(&self.conn(), correction.invoice_id) {
                correction.number = number.unwrap_or_default();
                correction.issue_date = data.issue_date;
            }
        }
    }

    fn correction_draft(
        &self,
        id: i64,
        kind: CorrectionKind,
        today: &str,
    ) -> Result<StoredInvoice> {
        let invoice = self.invoice(id)?;
        let mut draft = invoice.draft.clone();
        draft.issue_date = today.into();
        draft.frozen = None;
        draft.frozen_layout = None;
        draft.frozen_offers = Vec::new();
        draft.corrects = Some(Correction {
            invoice_id: id,
            kind,
            number: invoice.number.clone().unwrap_or_default(),
            issue_date: invoice.draft.issue_date.clone(),
        });
        if kind == CorrectionKind::Cancel {
            // it takes back what was billed and what that deducted, billing nothing; a Storno of an
            // invoice on account is on account of nothing, so no Schlussrechnung deducts it
            draft.positions = mirrored(&invoice.draft.positions);
            draft.on_account = None;
        }
        // the list shows what it comes to from the start, as `invoice_save` keeps it
        let settings = self.settings(invoice.context_id)?;
        let total = crate::draft::build_invoice(&draft, &settings, "ENTWURF")
            .ok()
            .map(|built| crate::draft::totals(&built).due);
        let json = serde_json::to_string(&draft)?;
        let new = {
            let conn = self.conn();
            let tx = conn.unchecked_transaction()?;
            check_correction(&tx, invoice.case_id, None, Kind::Invoice, &draft, false)?;
            let new = insert_draft(
                &tx,
                invoice.context_id,
                Kind::Invoice,
                &json,
                total.as_deref(),
                Some(id),
                invoice.case_id,
            )?;
            tx.commit()?;
            new
        };
        self.invoice(new)
    }

    /// A Stornorechnung for a created invoice, dated `today`: a draft that mirrors it
    pub fn cancel_invoice(&self, id: i64, today: &str) -> Result<StoredInvoice> {
        let invoice = self.invoice(id)?;
        if invoice.cancelled {
            bail!("Diese Rechnung ist schon storniert.");
        }
        // one draft at a time, it is opened again
        if let Some(draft) = invoice.cancel_draft {
            return self.invoice(draft);
        }
        self.correction_draft(id, CorrectionKind::Cancel, today)
    }

    /// An Ersatzrechnung for a cancelled invoice, dated `today`: a draft of what it said, to change
    pub fn replace_invoice(&self, id: i64, today: &str) -> Result<StoredInvoice> {
        if !self.invoice(id)?.cancelled {
            bail!("Ersetzt wird eine stornierte Rechnung; erst die Stornorechnung erstellen.");
        }
        self.correction_draft(id, CorrectionKind::Replace, today)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::tests::bake;
    use crate::context::Context;
    use crate::draft::{
        build_invoice,
        fixtures::{draft, settings},
    };
    use crate::model::PositionSource;

    fn store() -> Store {
        let store = Store::in_memory().unwrap();
        store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        store
    }

    fn created(store: &Store) -> StoredInvoice {
        let saved = store
            .save_draft(None, 1, Kind::Invoice, &draft(), Some("1181.08"))
            .unwrap();
        bake(store, saved.id, "RE-1", "1181.08");
        store.invoice(saved.id).unwrap()
    }

    #[test]
    fn a_stornorechnung_takes_the_invoice_back() {
        let store = store();
        let invoice = created(&store);
        let storno = store.cancel_invoice(invoice.id, "2026-11-02").unwrap();
        assert_eq!(storno.case_id, invoice.case_id);
        assert!(storno.total.as_deref().is_some_and(|t| t.starts_with('-')));
        // the same positions, their quantities taken back, the prices as they were
        let quantities: Vec<_> = storno
            .draft
            .positions
            .iter()
            .map(|p| p.quantity.clone())
            .collect();
        let taken_back: Vec<_> = invoice
            .draft
            .positions
            .iter()
            .map(|p| Some(negated(p.quantity.as_deref())))
            .collect();
        assert_eq!(quantities, taken_back);
        assert!(quantities.iter().flatten().all(|q| q.starts_with('-')));
        assert_eq!(
            storno.draft.positions[0].price,
            invoice.draft.positions[0].price
        );
        let correction = storno.draft.corrects.clone().unwrap();
        assert_eq!(
            (correction.kind, correction.number.as_str()),
            (CorrectionKind::Cancel, "RE-1")
        );
        // not cancelled before it is created, but it knows its draft, which is not made twice
        assert!(!store.invoice(invoice.id).unwrap().cancelled);
        assert_eq!(
            store.invoice(invoice.id).unwrap().cancel_draft,
            Some(storno.id)
        );
        assert_eq!(
            store.cancel_invoice(invoice.id, "2026-11-02").unwrap().id,
            storno.id
        );

        let built = build_invoice(&storno.draft, &settings(), "RE-2").unwrap();
        assert_eq!(
            built.type_code.as_ref().map(en16931::invoice::Code::as_str),
            Some("384")
        );
        assert_eq!(built.preceding_invoices[0].reference.as_str(), "RE-1");
        let original = build_invoice(&invoice.draft, &settings(), "RE-1").unwrap();
        assert_eq!(
            crate::draft::totals(&built).due,
            format!("-{}", crate::draft::totals(&original).due)
        );

        // it mirrors: no other positions, the same time of service
        let mut changed = storno.draft.clone();
        changed.positions[0].quantity = Some("-1".into());
        assert!(store
            .save_draft(Some(storno.id), 1, Kind::Invoice, &changed, None)
            .is_err());
        let mut moved = storno.draft.clone();
        moved.delivery_end = Some("2026-12-31".into());
        assert!(store
            .save_draft(Some(storno.id), 1, Kind::Invoice, &moved, None)
            .is_err());

        bake(&store, storno.id, "RE-2", "-1181.08");
        let cancelled = store.invoice(invoice.id).unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(
            cancelled.cancelled_by.unwrap().number.as_deref(),
            Some("RE-2")
        );
        assert_eq!(cancelled.cancel_draft, None);
        assert!(store.cancel_invoice(invoice.id, "2026-11-03").is_err());
        // a Stornorechnung is not cancelled itself
        assert!(store.cancel_invoice(storno.id, "2026-11-03").is_err());
        // the Vorgang counts neither
        assert_eq!(store.case(invoice.case_id).unwrap().invoiced, "0");
    }

    #[test]
    fn a_stornorechnung_asks_for_no_payment_on_paper() {
        let store = store();
        let invoice = created(&store);
        let storno = store.cancel_invoice(invoice.id, "2026-11-02").unwrap();
        let built = build_invoice(&storno.draft, &settings(), "RE-2").unwrap();
        let branding =
            crate::render::Branding::for_document(Kind::Invoice, &storno.draft, &settings(), &[]);
        let view = crate::render::build(&built, &branding, &crate::model::Format::default());
        assert_eq!(view.title, "Stornorechnung");
        assert!(view.payment.is_empty());
        let keys: Vec<_> = view
            .infos
            .iter()
            .map(|i| (i.key, i.label.as_str()))
            .collect();
        assert!(!keys.iter().any(|(key, _)| *key == "due-date"));
        assert!(keys.contains(&("preceding-invoice", "Storniert Rechnung")));
        assert_eq!(
            view.totals.last().map(|t| t.label.as_str()),
            Some("Stornobetrag")
        );
        // the XML still has its due date (BR-CO-25)
        assert!(built.due_date.is_some());
    }

    #[test]
    fn an_ersatzrechnung_replaces_a_cancelled_invoice() {
        let store = store();
        let invoice = created(&store);
        assert!(store.replace_invoice(invoice.id, "2026-11-02").is_err());
        let storno = store.cancel_invoice(invoice.id, "2026-11-02").unwrap();
        bake(&store, storno.id, "RE-2", "-1");
        let replacement = store.replace_invoice(invoice.id, "2026-11-02").unwrap();
        let built = build_invoice(&replacement.draft, &settings(), "RE-3").unwrap();
        assert_eq!(
            built.type_code.as_ref().map(en16931::invoice::Code::as_str),
            Some("380")
        );
        assert_eq!(built.preceding_invoices[0].reference.as_str(), "RE-1");
        // it may change what it says
        let mut changed = replacement.draft.clone();
        changed.positions[0].price = "99".into();
        store
            .save_draft(Some(replacement.id), 1, Kind::Invoice, &changed, None)
            .unwrap();
    }

    #[test]
    fn negated_quantities_read_as_typed() {
        assert_eq!(negated(None), "-1");
        assert_eq!(negated(Some(" ")), "-1");
        assert_eq!(negated(Some("2,5")), "-2,5");
        assert_eq!(negated(Some("-3")), "3");
    }

    #[test]
    fn a_stornorechnung_of_a_schlussrechnung_takes_its_deductions_back() {
        use crate::model::{Deduction, OnAccount};
        let store = store();
        let mut on_account = draft();
        on_account.on_account = Some(OnAccount::Progress);
        let abschlag = store
            .save_draft(None, 1, Kind::Invoice, &on_account, None)
            .unwrap();
        bake(&store, abschlag.id, "RE-1", "1");
        let mut closing = draft();
        closing.deductions = vec![Deduction {
            invoice_id: abschlag.id,
            number: String::new(),
            issue_date: String::new(),
            net: String::new(),
            vat: String::new(),
            gross: String::new(),
        }];
        let closing = store
            .save_draft_in_case(abschlag.case_id, Kind::Invoice, &closing, None)
            .unwrap();
        bake(&store, closing.id, "RE-2", "1");
        let closing = store.invoice(closing.id).unwrap();
        let built = build_invoice(&closing.draft, &settings(), "RE-2").unwrap();
        let due = crate::draft::totals(&built).due;

        let storno = store.cancel_invoice(closing.id, "2026-11-02").unwrap();
        assert_eq!(storno.draft.deductions, closing.draft.deductions);
        // what is taken back is what the Schlussrechnung asked for, not its full amount
        assert_eq!(storno.total, Some(format!("-{due}")));
        // created, it frees the Abschlag for the Ersatzrechnung
        bake(&store, storno.id, "RE-3", "-1");
        let replacement = store.replace_invoice(closing.id, "2026-11-03").unwrap();
        store
            .save_draft(
                Some(replacement.id),
                1,
                Kind::Invoice,
                &replacement.draft,
                None,
            )
            .unwrap();
        // the Abschlag itself is not cancelled by any of this
        assert!(!store.invoice(abschlag.id).unwrap().cancelled);
    }

    #[test]
    fn a_cancelled_invoice_bills_its_offer_positions_no_more() {
        let store = store();
        let offer = store
            .save_draft(None, 1, Kind::Offer, &draft(), None)
            .unwrap();
        bake(&store, offer.id, "AN-1", "1");
        let invoice = store
            .invoice_from_offer(offer.id, "2026-11-02", &settings())
            .unwrap();
        bake(&store, invoice.id, "RE-1", "1");
        assert!(store
            .invoice_from_offer(offer.id, "2026-11-03", &settings())
            .is_err());
        let storno = store.cancel_invoice(invoice.id, "2026-11-03").unwrap();
        // the Storno bills nothing
        assert!(storno.draft.positions.iter().all(|p| p.source.is_none()));
        bake(&store, storno.id, "RE-2", "-1");
        let again = store
            .invoice_from_offer(offer.id, "2026-11-04", &settings())
            .unwrap();
        assert_eq!(
            again.draft.positions[0].source,
            Some(PositionSource {
                offer_id: offer.id,
                index: 0
            })
        );
    }
}
