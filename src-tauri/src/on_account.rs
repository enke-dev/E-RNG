//! Invoices on account and the Schlussrechnung. An Anzahlungsrechnung asks for money before any
//! work, an Abschlagsrechnung for the work so far (`InvoiceDraft.on_account`); both are payments on
//! account, they bill no offer position for good. A Schlussrechnung is an invoice that deducts the
//! created invoices on account of its Vorgang (`InvoiceDraft.deductions`): it states its full
//! amounts, names each of them as a preceding invoice and takes what they came to off what is due.
//! Each invoice on account is deducted once.

use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::draft::{build_invoice, totals};
use crate::model::{InvoiceDraft, Kind, OnAccount};
use crate::store::{context_settings, Store};

/// A created invoice on account of a Vorgang, with what it came to and what deducts it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnAccountInvoice {
    pub id: i64,
    pub number: String,
    pub issue_date: String,
    pub on_account: OnAccount,
    pub net: String,
    pub vat: String,
    pub gross: String,
    /// The number of the created invoice that deducts it, or `Entwurf` for a draft that does
    pub deducted_by: Option<String>,
    /// What was received on it so far (none without payments), and the day it was paid completely:
    /// a Schlussrechnung deducts it as invoiced and warns while it is not
    pub paid: Option<String>,
    pub paid_at: Option<String>,
}

/// A created invoice on account as a deduction names it: number, day, net, VAT, gross
fn figures(conn: &Connection, id: i64) -> Result<Option<OnAccountInvoice>> {
    let row: Option<(String, String, i64, Option<String>, String)> = conn
        .query_row(
            "SELECT kind, status, context_id, number, draft FROM invoices WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((kind, status, context, number, draft)) = row else {
        return Ok(None);
    };
    let draft: InvoiceDraft = serde_json::from_str(&draft)?;
    let (Some(on_account), Some(number)) = (draft.on_account, number) else {
        return Ok(None);
    };
    if kind != Kind::Invoice.as_str() || status != "exported" {
        return Ok(None);
    }
    // a cancelled one is deducted by nothing
    let cancelled: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM invoices
         WHERE status = 'exported' AND json_extract(draft, '$.corrects.kind') = 'cancel'
             AND json_extract(draft, '$.corrects.invoice_id') = ?1",
        [id],
        |r| r.get(0),
    )?;
    if cancelled {
        return Ok(None);
    }
    // a created invoice froze its settings, so it is built again exactly as it was
    let settings = context_settings(conn, context).unwrap_or_default();
    let Ok(invoice) = build_invoice(&draft, &settings, &number) else {
        bail!("Die Abschlagsrechnung {number} lässt sich nicht mehr berechnen.");
    };
    let t = totals(&invoice);
    let (paid, paid_at) =
        crate::history::settle(Some(&t.gross), &crate::history::payments_of(conn, id)?);
    Ok(Some(OnAccountInvoice {
        id,
        number,
        issue_date: draft.issue_date,
        on_account,
        net: t.net,
        vat: t.vat,
        gross: t.gross,
        deducted_by: None,
        paid,
        paid_at,
    }))
}

/// SQL over an invoice `i` whose deductions count: no Stornorechnung (it takes them back) and not
/// cancelled by a created one (they are free again for its Ersatzrechnung)
const DEDUCTING: &str = "json_extract(i.draft, '$.corrects.kind') IS NOT 'cancel'
     AND NOT EXISTS (SELECT 1 FROM invoices s WHERE s.status = 'exported'
         AND json_extract(s.draft, '$.corrects.kind') = 'cancel'
         AND json_extract(s.draft, '$.corrects.invoice_id') = i.id)";

/// Who deducts the invoice on account `id`, leaving `except` out: a created invoice by its
/// number, else a draft
fn deducted_by(conn: &Connection, id: i64, except: Option<i64>) -> Result<Option<String>> {
    let mut statement = conn.prepare(&format!(
        "SELECT i.id, i.number FROM invoices i, json_each(i.draft, '$.deductions') d
         WHERE i.kind = 'invoice' AND json_extract(d.value, '$.invoice_id') = ?1 AND {DEDUCTING}
         ORDER BY i.number IS NULL, i.id"
    ))?;
    let rows = statement
        .query_map([id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .find(|(other, _)| Some(*other) != except)
        .map(|(_, number)| number.unwrap_or_else(|| "Entwurf".into())))
}

/// The deductions of a Schlussrechnung must be created invoices on account of its Vorgang that no
/// created invoice deducts yet, each once; an invoice on account deducts nothing and bills no offer
/// position
pub(crate) fn check_deductions(
    conn: &Connection,
    case_id: i64,
    id: Option<i64>,
    kind: Kind,
    draft: &InvoiceDraft,
) -> Result<()> {
    if draft.deductions.is_empty() {
        return Ok(());
    }
    if kind != Kind::Invoice || draft.on_account.is_some() {
        bail!("Nur eine Schlussrechnung zieht Abschlags- und Anzahlungsrechnungen ab.");
    }
    // a Stornorechnung takes back what its invoice deducted, `check_correction` holds it to that
    if crate::draft::cancels(draft) {
        return Ok(());
    }
    let mut seen = std::collections::BTreeSet::new();
    for deduction in &draft.deductions {
        if !seen.insert(deduction.invoice_id) {
            bail!("Eine Abschlagsrechnung wird nur einmal abgezogen.");
        }
        let row: Option<(i64, String)> = conn
            .query_row(
                "SELECT case_id, status FROM invoices WHERE id = ?1",
                [deduction.invoice_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let in_case = row.is_some_and(|(case, status)| case == case_id && status == "exported");
        if !in_case || figures(conn, deduction.invoice_id)?.is_none() {
            bail!("Abgezogen werden nur erstellte Abschlags- und Anzahlungsrechnungen desselben Vorgangs.");
        }
        // a draft that deducts it too is fine until one of them is created
        let created_elsewhere: bool = conn.query_row(
            &format!(
                "SELECT COUNT(*) > 0 FROM invoices i, json_each(i.draft, '$.deductions') d
                 WHERE i.status = 'exported' AND i.id IS NOT ?2
                     AND json_extract(d.value, '$.invoice_id') = ?1 AND {DEDUCTING}"
            ),
            params![deduction.invoice_id, id],
            |r| r.get(0),
        )?;
        if created_elsewhere {
            bail!(
                "Die Abschlagsrechnung {} zieht schon eine andere Schlussrechnung ab.",
                deduction.number
            );
        }
    }
    Ok(())
}

/// An invoice on account bills no offer position for good
pub(crate) fn check_on_account(draft: &InvoiceDraft) -> Result<()> {
    if draft.on_account.is_some() && draft.positions.iter().any(|p| p.source.is_some()) {
        bail!(
            "Eine Abschlags- oder Anzahlungsrechnung berechnet keine Angebotsposition endgültig."
        );
    }
    Ok(())
}

impl Store {
    /// Fills in what the deducted invoices came to, from the invoices themselves: number, day, net,
    /// VAT and gross. An unknown one is left as it is, the save refuses it.
    pub fn fill_deductions(&self, draft: &mut InvoiceDraft) -> Result<()> {
        let conn = self.conn();
        for deduction in &mut draft.deductions {
            if let Some(found) = figures(&conn, deduction.invoice_id)? {
                deduction.number = found.number;
                deduction.issue_date = found.issue_date;
                deduction.net = found.net;
                deduction.vat = found.vat;
                deduction.gross = found.gross;
            }
        }
        Ok(())
    }

    /// The created invoices on account of a Vorgang, oldest first, with what deducts each; `except`
    /// leaves the asking draft out
    pub fn case_on_account(
        &self,
        case_id: i64,
        except: Option<i64>,
    ) -> Result<Vec<OnAccountInvoice>> {
        let conn = self.conn();
        let ids: Vec<i64> = {
            let mut statement = conn.prepare(
                "SELECT id FROM invoices WHERE case_id = ?1 AND kind = 'invoice'
                     AND status = 'exported' ORDER BY id",
            )?;
            let ids = statement
                .query_map([case_id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        };
        ids.into_iter()
            .filter_map(|id| figures(&conn, id).transpose())
            .map(|found| {
                let mut found = found?;
                found.deducted_by = deducted_by(&conn, found.id, except)?;
                Ok(found)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::tests::bake;
    use crate::context::Context;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::{Deduction, Position};

    fn store() -> Store {
        let store = Store::in_memory().unwrap();
        store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        store
    }

    fn one(description: &str, price: &str) -> Vec<Position> {
        vec![Position {
            description: description.into(),
            quantity: None,
            price: price.into(),
            ..Position::default()
        }]
    }

    /// A created Abschlag of `net` in a Vorgang, or a new one
    fn progress(
        store: &Store,
        case: Option<i64>,
        number: &str,
        net: &str,
    ) -> crate::model::StoredInvoice {
        let mut on_account = draft();
        on_account.on_account = Some(OnAccount::Progress);
        on_account.positions = one("Abschlag 30 %", net);
        let saved = match case {
            Some(case) => store.save_draft_in_case(case, Kind::Invoice, &on_account, None),
            None => store.save_draft(None, 1, Kind::Invoice, &on_account, None),
        }
        .unwrap();
        // stored as what it asks for, 19 % on the net
        let gross = (crate::draft::decimal(net).unwrap() * rust_decimal::Decimal::new(119, 2))
            .round_dp(2)
            .to_string();
        bake(store, saved.id, number, &gross);
        store.invoice(saved.id).unwrap()
    }

    fn deducting(ids: &[i64]) -> InvoiceDraft {
        let mut closing = draft();
        closing.positions = one("Gesamtleistung", "1000");
        closing.deductions = ids
            .iter()
            .map(|&invoice_id| Deduction {
                invoice_id,
                number: String::new(),
                issue_date: String::new(),
                net: String::new(),
                vat: String::new(),
                gross: String::new(),
            })
            .collect();
        closing
    }

    #[test]
    fn a_schlussrechnung_is_one_once_picked() {
        let mut picked = draft();
        assert!(!picked.is_closing());
        // before it deducts anything
        picked.settles = true;
        assert!(picked.is_closing());
        picked.on_account = Some(OnAccount::Progress);
        assert!(!picked.is_closing());
        // deducting makes one too
        let deducting = deducting(&[1]);
        assert!(deducting.is_closing());
    }

    #[test]
    fn an_invoice_on_account_says_what_it_is_in_the_xml() {
        let mut on_account = draft();
        on_account.on_account = Some(OnAccount::Prepayment);
        let invoice = build_invoice(&on_account, &settings(), "RE-1").unwrap();
        assert_eq!(
            invoice
                .type_code
                .as_ref()
                .map(en16931::invoice::Code::as_str),
            Some("386")
        );
        // before any work: no date of service
        assert!(invoice.delivery.is_none() && invoice.invoicing_period.is_none());
        // XRechnung has no prepayment invoice, it goes as a partial one
        on_account.profile = crate::invoice::ProfileKind::XRechnung;
        let invoice = build_invoice(&on_account, &settings(), "RE-1").unwrap();
        assert_eq!(
            invoice
                .type_code
                .as_ref()
                .map(en16931::invoice::Code::as_str),
            Some("326")
        );
        on_account.on_account = Some(OnAccount::Progress);
        let invoice = build_invoice(&on_account, &settings(), "RE-1").unwrap();
        assert_eq!(
            invoice
                .type_code
                .as_ref()
                .map(en16931::invoice::Code::as_str),
            Some("326")
        );
        assert!(invoice.delivery.is_some());
    }

    #[test]
    fn a_schlussrechnung_deducts_the_invoices_on_account_of_its_vorgang() {
        let store = store();
        let first = progress(&store, None, "RE-1", "300");
        let second = progress(&store, Some(first.case_id), "RE-2", "200");
        let listed = store.case_on_account(first.case_id, None).unwrap();
        assert_eq!(
            listed
                .iter()
                .map(|l| (l.number.as_str(), l.net.as_str(), l.gross.as_str()))
                .collect::<Vec<_>>(),
            [("RE-1", "300.00", "357.00"), ("RE-2", "200.00", "238.00")]
        );
        // what was received on them, for the warning on what is not paid yet
        store.add_payment(first.id, "100", "2026-11-05").unwrap();
        let listed = store.case_on_account(first.case_id, None).unwrap();
        assert_eq!(
            (listed[0].paid.as_deref(), listed[0].paid_at.as_deref()),
            (Some("100"), None)
        );
        assert_eq!(listed[1].paid, None);

        let mut closing = deducting(&[first.id, second.id]);
        store.fill_deductions(&mut closing).unwrap();
        assert_eq!(closing.deductions[0].number, "RE-1");
        assert_eq!(closing.deductions[1].vat, "38.00");
        let saved = store
            .save_draft_in_case(first.case_id, Kind::Invoice, &closing, None)
            .unwrap();
        // the full amounts, the deductions off what is due, each named as a preceding invoice
        let invoice = build_invoice(&saved.draft, &settings(), "RE-3").unwrap();
        let t = totals(&invoice);
        assert_eq!((t.gross.as_str(), t.due.as_str()), ("1190.00", "595.00"));
        assert_eq!(
            invoice
                .preceding_invoices
                .iter()
                .map(|p| p.reference.as_str())
                .collect::<Vec<_>>(),
            ["RE-1", "RE-2"]
        );
        assert_eq!(
            store
                .case_on_account(first.case_id, Some(saved.id))
                .unwrap()[0]
                .deducted_by,
            None
        );
        assert_eq!(
            store.case_on_account(first.case_id, None).unwrap()[0]
                .deducted_by
                .as_deref(),
            Some("Entwurf")
        );
        bake(&store, saved.id, "RE-3", "595");
        assert_eq!(
            store.case_on_account(first.case_id, None).unwrap()[1]
                .deducted_by
                .as_deref(),
            Some("RE-3")
        );
        // not twice
        let mut again = deducting(&[first.id]);
        store.fill_deductions(&mut again).unwrap();
        assert!(store
            .save_draft_in_case(first.case_id, Kind::Invoice, &again, None)
            .is_err());
    }

    #[test]
    fn a_schlussrechnung_shows_each_deduction_with_its_vat_and_what_is_left() {
        let store = store();
        let first = progress(&store, None, "RE-1", "300");
        let mut closing = deducting(&[first.id]);
        store.fill_deductions(&mut closing).unwrap();
        let invoice = build_invoice(&closing, &settings(), "RE-2").unwrap();
        let branding =
            crate::render::Branding::for_document(Kind::Invoice, &closing, &settings(), &[]);
        assert_eq!(branding.wording.title, "Schlussrechnung");
        let view = crate::render::build(&invoice, &branding, &crate::model::Format::default());
        let rows: Vec<_> = view
            .totals
            .iter()
            .map(|t| (t.label.as_str(), t.value.as_str()))
            .collect();
        assert!(rows.contains(&("Gesamtbetrag", "1.190,00\u{a0}€")));
        assert!(rows
            .iter()
            .any(|(label, value)| label.starts_with("Abzüglich RE-1 vom")
                && label.contains("netto 300,00\u{a0}€, USt. 57,00\u{a0}€)")
                && value.ends_with("357,00\u{a0}€")));
        assert_eq!(rows.last(), Some(&("Zahlbetrag", "833,00\u{a0}€")));
        assert!(!rows.iter().any(|(label, _)| *label == "Bereits bezahlt"));

        let mut on_account = draft();
        on_account.on_account = Some(OnAccount::Prepayment);
        let words =
            crate::render::Branding::for_document(Kind::Invoice, &on_account, &settings(), &[])
                .wording;
        assert_eq!(
            (words.title.as_str(), words.draft_title.as_str()),
            ("Anzahlungsrechnung", "Anzahlungsrechnung (Entwurf)")
        );
    }

    #[test]
    fn only_created_invoices_on_account_of_the_vorgang_are_deducted() {
        let store = store();
        let first = progress(&store, None, "RE-1", "300");
        let elsewhere = progress(&store, None, "RE-2", "100");
        let plain = store
            .save_draft_in_case(first.case_id, Kind::Invoice, &draft(), None)
            .unwrap();
        bake(&store, plain.id, "RE-3", "1");
        for ids in [
            vec![elsewhere.id],
            vec![plain.id],
            vec![first.id, first.id],
            vec![999],
        ] {
            assert!(store
                .save_draft_in_case(first.case_id, Kind::Invoice, &deducting(&ids), None)
                .is_err());
        }
        // an invoice on account deducts nothing and bills no offer position
        let mut both = deducting(&[first.id]);
        both.on_account = Some(OnAccount::Progress);
        assert!(store
            .save_draft_in_case(first.case_id, Kind::Invoice, &both, None)
            .is_err());
        let mut sourced = draft();
        sourced.on_account = Some(OnAccount::Prepayment);
        sourced.positions[0].source = Some(crate::model::PositionSource {
            offer_id: 1,
            index: 0,
        });
        assert!(check_on_account(&sourced).is_err());
    }
}
