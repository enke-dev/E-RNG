//! Turns what the form holds into the invoice model the rules and the XML are built on.
//! This is the one place that knows how positions, customer and settings map to EN 16931 terms.

use std::str::FromStr;

use en16931::codes::guard;
use en16931::invoice::{
    Contact, Delivery, InvoiceNote, Party, PaymentInstructions, PaymentMeans, Period, PostalAddress,
};
use en16931::{Date, Identifier, Percentage, Quantity, Reconciler, UnitPriceAmount};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::invoice::{line_from_unit_price, Invoice, ProfileKind};
use crate::model::{Address, Customer, InvoiceDraft, Position, Seller, Settings};

/// The Peppol business process `XRechnung` asks for (BT-23)
const BUSINESS_PROCESS: &str = "urn:fdc:peppol.eu:2017:poacc:billing:01:1.0";

/// Totals as the invoice will show them, for the form's live summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub net: String,
    pub vat: String,
    pub gross: String,
}

/// Input that cannot even be turned into an invoice (a price that is no number, a missing date),
/// as opposed to findings, which are about an invoice that exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputProblem {
    /// Which field, e.g. `positions[1].price`
    pub field: String,
    pub message: String,
}

fn problem(field: impl Into<String>, message: impl Into<String>) -> InputProblem {
    InputProblem {
        field: field.into(),
        message: message.into(),
    }
}

/// The value of a parse that went well; what went wrong is noted for the user.
fn noted<T>(result: Result<T, InputProblem>, problems: &mut Vec<InputProblem>) -> Option<T> {
    result.map_err(|p| problems.push(p)).ok()
}

fn blank(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|t| !t.is_empty())
}

/// Accepts `1.234,50` and `1234.50` alike, the webview may hand over either.
pub(crate) fn decimal(text: &str) -> Option<Decimal> {
    let trimmed = text.trim();
    let normalized = if trimmed.contains(',') {
        trimmed.replace('.', "").replace(',', ".")
    } else {
        trimmed.to_owned()
    };
    Decimal::from_str(&normalized).ok()
}

fn address(field: &str, address: &Address) -> Result<PostalAddress, InputProblem> {
    let mut postal = PostalAddress::new(
        address.street.trim(),
        address.city.trim(),
        address.post_code.trim(),
        address.country.trim(),
    )
    .map_err(|_| {
        problem(
            format!("{field}.country"),
            "Kein gültiger Ländercode (z. B. DE)",
        )
    })?;
    // what was not given is left out: an empty element in the XML is an error of its own
    // (PEPPOL-EN16931-R008)
    for part in [&mut postal.line1, &mut postal.city, &mut postal.post_code] {
        part.take_if(|p| p.is_empty());
    }
    Ok(postal)
}

fn electronic(field: &str, email: &str) -> Result<Identifier, InputProblem> {
    Identifier::eas(email.trim(), "EM").map_err(|_| problem(field, "Keine gültige E-Mail-Adresse"))
}

fn seller_party(seller: &Seller) -> Result<Party, Vec<InputProblem>> {
    let mut problems = Vec::new();
    let postal = noted(address("seller.address", &seller.address), &mut problems);
    let mut party = Party::new(seller.name.trim(), postal.unwrap_or_default());
    match (
        blank(seller.vat_id.as_deref()),
        blank(seller.tax_number.as_deref()),
    ) {
        (Some(vat), _) => party = party.with_vat_identifier(vat),
        // BR-CO-26 wants a seller identifier or a VAT id, a tax number (BT-32) alone does not count
        (None, Some(tax)) => party.identifiers.push(Identifier::new(tax)),
        (None, None) => {}
    }
    party.tax_registration = blank(seller.tax_number.as_deref()).map(str::to_owned);
    party.additional_legal_information = blank(seller.legal_info.as_deref()).map(str::to_owned);
    party = party.with_contact(Contact {
        name: Some(seller.contact_name.trim().to_owned()).filter(|n| !n.is_empty()),
        phone: Some(seller.phone.trim().to_owned()).filter(|n| !n.is_empty()),
        email: Some(seller.email.trim().to_owned()).filter(|n| !n.is_empty()),
    });
    if !seller.email.trim().is_empty() {
        match electronic("seller.email", &seller.email) {
            Ok(id) => party = party.with_electronic_address(id),
            Err(p) => problems.push(p),
        }
    }
    if problems.is_empty() {
        Ok(party)
    } else {
        Err(problems)
    }
}

fn buyer_party(customer: &Customer) -> Result<Party, Vec<InputProblem>> {
    let mut problems = Vec::new();
    let postal = noted(
        address("customer.address", &customer.address),
        &mut problems,
    );
    let mut party = Party::new(customer.name.trim(), postal.unwrap_or_default());
    if let Some(vat) = blank(customer.vat_id.as_deref()) {
        party = party.with_vat_identifier(vat);
    }
    if let Some(email) = blank(customer.email.as_deref()) {
        match electronic("customer.email", email) {
            Ok(id) => party = party.with_electronic_address(id),
            Err(p) => problems.push(p),
        }
    }
    if problems.is_empty() {
        Ok(party)
    } else {
        Err(problems)
    }
}

fn payment(seller: &Seller) -> PaymentInstructions {
    let mut payment = PaymentInstructions::sepa_credit_transfer(
        seller.iban.replace(' ', ""),
        blank(seller.bic.as_deref()),
    );
    if let Some(PaymentMeans::CreditTransfer(transfers)) = &mut payment.means {
        for transfer in transfers {
            transfer.account_name = blank(seller.account_holder.as_deref()).map(str::to_owned);
        }
    }
    payment
}

fn line(
    index: usize,
    position: &Position,
    rate: Percentage,
    vat_free: bool,
) -> Result<en16931::InvoiceLine, Vec<InputProblem>> {
    let field = |name: &str| format!("positions[{index}].{name}");
    let mut problems = Vec::new();

    let quantity = match blank(position.quantity.as_deref()) {
        None => Some(Decimal::ONE),
        Some(text) => {
            let parsed = decimal(text).filter(|q| !q.is_zero());
            if parsed.is_none() {
                problems.push(problem(field("quantity"), "Keine gültige Menge"));
            }
            parsed
        }
    };
    let price = decimal(&position.price);
    if price.is_none() {
        problems.push(problem(field("price"), "Ein Preis ist erforderlich"));
    }
    if position.description.trim().is_empty() {
        problems.push(problem(
            field("description"),
            "Eine Beschreibung ist erforderlich",
        ));
    }
    let code = position.unit_code();
    if guard::unit(code).is_err() {
        problems.push(problem(
            field("unit"),
            format!("Der Einheitencode „{code}“ ist nicht zulässig"),
        ));
    }
    let (Some(quantity), Some(price)) = (quantity, price) else {
        return Err(problems);
    };
    if !problems.is_empty() {
        return Err(problems);
    }

    line_from_unit_price(
        &(index + 1).to_string(),
        position.description.trim(),
        Quantity::new(quantity),
        position.unit_code(),
        UnitPriceAmount::new(price),
        if vat_free { "E" } else { "S" },
        Some(rate),
    )
    .map_err(|_| vec![problem(field("price"), "Der Betrag ist zu groß")])
}

/// The invoice for a draft, with the settings in force (or, for exported ones, frozen).
///
/// `number` is the final one, or a placeholder while the draft is only being checked.
pub fn build_invoice(
    draft: &InvoiceDraft,
    current: &Settings,
    number: &str,
) -> Result<Invoice, Vec<InputProblem>> {
    let settings = draft.frozen.as_ref().unwrap_or(current);
    let mut problems = Vec::new();

    let issue = date("issue_date", &draft.issue_date, &mut problems);
    let service = service_time(draft, &mut problems);
    // no rate is no VAT: exempt (E) with 0 %, and the reason on the invoice
    let vat_free = settings.vat_free();
    let rate = if vat_free {
        Some(Percentage::new(Decimal::ZERO))
    } else {
        decimal(&settings.tax_rate)
            .filter(|r| *r > Decimal::ZERO && *r <= Decimal::ONE_HUNDRED)
            .map(Percentage::new)
    };
    if rate.is_none() {
        problems.push(problem("tax_rate", "Kein gültiger Steuersatz"));
    }
    let seller = seller_party(&settings.seller)
        .map_err(|p| problems.extend(p))
        .ok();
    let buyer = buyer_party(&draft.customer)
        .map_err(|p| problems.extend(p))
        .ok();
    if draft.positions.is_empty() {
        problems.push(problem(
            "positions",
            "Mindestens eine Position ist erforderlich",
        ));
    }

    let mut lines = Vec::new();
    if let Some(rate) = rate {
        for (index, position) in draft.positions.iter().enumerate() {
            match line(index, position, rate, vat_free) {
                Ok(line) => lines.push(line),
                Err(p) => problems.extend(p),
            }
        }
    }
    let (Some(issue), Some((delivery, delivery_end)), Some(seller), Some(buyer)) =
        (issue, service, seller, buyer)
    else {
        return Err(problems);
    };
    if !problems.is_empty() {
        return Err(problems);
    }

    let mut builder = Invoice::builder(
        draft.profile.specification_id(),
        number,
        issue,
        "380",
        "EUR",
    )
    .seller(seller)
    .buyer(buyer)
    .payment(payment(&settings.seller));
    // one day is the date of service (BT-72), a period the invoicing period (BG-14)
    builder = match delivery_end {
        Some(end) => builder.invoicing_period(Period {
            start: Some(delivery),
            end: Some(end),
        }),
        None => builder.delivery(Delivery {
            date: Some(delivery),
            ..Delivery::default()
        }),
    };
    // EN 16931 wants a due date or payment terms (BR-CO-25): without terms the invoice is due when
    // it is issued, which the document does not need to say
    builder = match settings.payment_days {
        Some(days) => builder
            .due_in_days(i32::try_from(days).unwrap_or(14))
            .payment_terms(format!("Zahlbar innerhalb von {days} Tagen ohne Abzug")),
        None => builder.due_in_days(0),
    };
    if let Some(reference) = blank(draft.buyer_reference.as_deref()) {
        builder = builder.buyer_reference(reference);
    }
    if draft.profile == ProfileKind::XRechnung {
        builder = builder.business_process(BUSINESS_PROCESS);
    }
    if let Some(note) = blank(draft.note.as_deref()) {
        builder = builder.note(InvoiceNote::new(note));
    }
    for line in lines {
        builder = builder.line(line);
    }
    let reconciler = if vat_free {
        Reconciler::new().exemption(
            "E",
            Some(settings.vat_note.trim()).filter(|n| !n.is_empty()),
            None,
        )
    } else {
        Reconciler::new()
    };
    builder.build_reconciled_with(&reconciler).map_err(|e| {
        vec![problem(
            "positions",
            format!("Die Summen lassen sich nicht berechnen: {e}"),
        )]
    })
}

/// The date of a field, `None` with the problem noted.
fn date(field: &str, text: &str, problems: &mut Vec<InputProblem>) -> Option<Date> {
    let parsed = Date::parse(text).ok();
    if parsed.is_none() {
        problems.push(problem(field, "Kein gültiges Datum"));
    }
    parsed
}

/// When the service was: one day (BT-72), or a period (BG-14) with its end; `None` with the
/// problems noted.
fn service_time(
    draft: &InvoiceDraft,
    problems: &mut Vec<InputProblem>,
) -> Option<(Date, Option<Date>)> {
    let start = date("delivery_date", &draft.delivery_date, problems);
    // a period chosen but its end not given yet is a missing date, not a single day
    let end = match draft.delivery_end.as_deref() {
        Some(end) => Some(date("delivery_end", end, problems)?),
        None => None,
    };
    let start = start?;
    if end.is_some_and(|end| end < start) {
        problems.push(problem(
            "delivery_end",
            "Das Ende des Leistungszeitraums liegt vor seinem Beginn",
        ));
        return None;
    }
    Some((start, end))
}

/// A draft the template can show while it is still being filled in: whatever would stop the invoice
/// from being built (an empty price, no description, a half typed date) gets a harmless stand-in.
/// Only for the preview, nothing built from this is ever stored or exported.
pub fn for_preview(draft: &InvoiceDraft) -> InvoiceDraft {
    let mut draft = draft.clone();
    let today = crate::utils::today();
    if Date::parse(&draft.issue_date).is_err() {
        draft.issue_date = today;
    }
    if Date::parse(&draft.delivery_date).is_err() {
        draft.delivery_date.clone_from(&draft.issue_date);
    }
    let start = Date::parse(&draft.delivery_date).ok();
    if let Some(end) = &mut draft.delivery_end {
        if Date::parse(end).ok().is_none_or(|e| Some(e) < start) {
            end.clone_from(&draft.delivery_date);
        }
    }
    if draft.positions.is_empty() {
        draft.positions.push(Position::default());
    }
    for position in &mut draft.positions {
        if decimal(&position.price).is_none() {
            position.price = "0".into();
        }
        if blank(position.quantity.as_deref())
            .is_some_and(|q| decimal(q).is_none_or(|q| q.is_zero()))
        {
            position.quantity = None;
        }
        if position.description.trim().is_empty() {
            position.description = "–".into();
        }
    }
    draft
}

pub fn totals(invoice: &Invoice) -> Totals {
    let t = &invoice.totals;
    Totals {
        net: t.taxable_total.to_string(),
        vat: t.vat_total.map_or_else(|| "0.00".into(), |v| v.to_string()),
        gross: t.gross_total.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub fn settings() -> Settings {
        Settings {
            seller: Seller {
                name: "Muster Software GmbH".into(),
                address: Address {
                    street: "Hauptstraße 5".into(),
                    post_code: "50667".into(),
                    city: "Köln".into(),
                    country: "DE".into(),
                },
                vat_id: Some("DE123456789".into()),
                tax_number: None,
                contact_name: "Max Muster".into(),
                phone: "+49 221 123456".into(),
                email: "max@muster.example".into(),
                mobile: None,
                fax: None,
                website: None,
                iban: "DE02 1203 0000 0000 2020 51".into(),
                bic: Some("BYLADEM1001".into()),
                account_holder: Some("Muster Software GmbH".into()),
                bank_name: None,
                legal_info: None,
            },
            ..Settings::default()
        }
    }

    pub fn customer() -> Customer {
        Customer {
            id: None,
            context_id: None,
            name: "Beispiel AG".into(),
            address: Address {
                street: "Marktplatz 2".into(),
                post_code: "54321".into(),
                city: "Beispielstadt".into(),
                country: "DE".into(),
            },
            vat_id: None,
            email: Some("einkauf@beispiel.example".into()),
            buyer_reference: Some("991-01234-56".into()),
        }
    }

    pub fn draft() -> InvoiceDraft {
        InvoiceDraft {
            customer: customer(),
            issue_date: "2026-10-17".into(),
            delivery_date: "2026-10-17".into(),
            delivery_end: None,
            validity_days: None,
            positions: vec![
                Position {
                    description: "Beratung".into(),
                    quantity: Some("10".into()),
                    unit: Some("HUR".into()),
                    unit_label: None,
                    price: "95,00".into(),
                },
                Position {
                    description: "Reisekosten".into(),
                    quantity: None,
                    unit: None,
                    unit_label: None,
                    price: "42.50".into(),
                },
            ],
            profile: ProfileKind::En16931,
            buyer_reference: None,
            note: None,
            layout: None,
            intro: None,
            closing: None,
            frozen: None,
            frozen_layout: None,
            offer_reference: false,
            frozen_offers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use crate::invoice::validate;

    #[test]
    fn a_complete_draft_becomes_a_valid_invoice() {
        let invoice = build_invoice(&draft(), &settings(), "RE-2026-001").unwrap();

        assert_eq!(validate(&invoice, ProfileKind::En16931), Vec::new());
        // 10 × 95,00 + 1 × 42,50 = 992,50 net, 19 % = 188,58 (rounded), gross 1.181,08
        let t = totals(&invoice);
        assert_eq!(
            (t.net.as_str(), t.vat.as_str(), t.gross.as_str()),
            ("992.50", "188.58", "1181.08")
        );
    }

    #[test]
    fn a_unit_code_the_rules_do_not_know_is_an_input_problem() {
        for (code, ok) in [("KWH", true), ("kwh", false), ("NOPE", false), (" ", true)] {
            let mut named = draft();
            named.positions[0].unit = Some(code.into());

            let built = build_invoice(&named, &settings(), "RE-1");

            match built {
                Ok(_) => assert!(ok, "{code:?} was accepted"),
                Err(problems) => {
                    assert!(!ok, "{code:?} was refused: {problems:?}");
                    assert_eq!(problems.len(), 1);
                    assert_eq!(problems[0].field, "positions[0].unit");
                    assert_eq!(
                        problems[0].message,
                        format!("Der Einheitencode „{code}“ ist nicht zulässig")
                    );
                }
            }
        }
    }

    #[test]
    fn without_a_rate_no_vat_is_charged_and_the_reason_is_on_the_invoice() {
        let free = Settings {
            tax_rate: " ".into(),
            vat_note: "Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.".into(),
            ..settings()
        };

        let invoice = build_invoice(&draft(), &free, "RE-1").unwrap();

        assert_eq!(validate(&invoice, ProfileKind::En16931), Vec::new());
        let t = totals(&invoice);
        assert_eq!(
            (t.net.as_str(), t.vat.as_str(), t.gross.as_str()),
            ("992.50", "0.00", "992.50")
        );
        assert!(invoice.lines.iter().all(|l| l.vat.category.as_str() == "E"));
        assert_eq!(
            invoice.vat_breakdown[0].exemption_reason.as_deref(),
            Some("Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.")
        );
        // a rate that is none or 0 is no rate either
        for bad in ["0", "abc", "101"] {
            let invalid = Settings {
                tax_rate: bad.into(),
                ..settings()
            };
            assert!(build_invoice(&draft(), &invalid, "X").is_err(), "{bad}");
        }
    }

    #[test]
    fn without_payment_days_the_invoice_is_due_when_issued_and_has_no_terms() {
        let none = Settings {
            payment_days: None,
            ..settings()
        };

        let invoice = build_invoice(&draft(), &none, "RE-1").unwrap();

        // EN 16931 wants a due date or terms (BR-CO-25), so the date is there, the terms are not
        assert_eq!(invoice.due_date, invoice.issue_date);
        assert_eq!(invoice.payment_terms, None);
        assert_eq!(validate(&invoice, ProfileKind::En16931), Vec::new());

        let with = build_invoice(&draft(), &settings(), "RE-1").unwrap();
        assert!(with.payment_terms.is_some() && with.due_date != with.issue_date);
    }

    #[test]
    fn quantity_defaults_to_one_and_comma_prices_work() {
        let invoice = build_invoice(&draft(), &settings(), "X").unwrap();
        let second = &invoice.lines[1];

        assert_eq!(second.quantity.to_string(), "1");
        assert_eq!(second.net_amount.to_string(), "42.50");
        assert_eq!(invoice.lines[0].net_amount.to_string(), "950.00");
    }

    #[test]
    fn xrechnung_drafts_carry_buyer_reference_and_process() {
        let mut d = draft();
        d.profile = ProfileKind::XRechnung;
        d.buyer_reference = Some("991-01234-56".into());
        d.customer.email = Some("einkauf@beispiel.example".into());

        let invoice = build_invoice(&d, &settings(), "RE-1").unwrap();

        assert!(
            validate(&invoice, ProfileKind::XRechnung).is_empty(),
            "{:#?}",
            validate(&invoice, ProfileKind::XRechnung)
        );
    }

    #[test]
    fn bad_input_is_reported_per_field() {
        let mut d = draft();
        d.positions[0].price = "abc".into();
        d.positions[1].description = "  ".into();
        d.positions[1].quantity = Some("0".into());
        d.issue_date = "17.10.2026".into();

        let problems = build_invoice(&d, &settings(), "X").unwrap_err();
        let fields: Vec<_> = problems.iter().map(|p| p.field.as_str()).collect();

        assert!(fields.contains(&"positions[0].price"));
        assert!(fields.contains(&"positions[1].description"));
        assert!(fields.contains(&"positions[1].quantity"));
        assert!(fields.contains(&"issue_date"));
    }

    #[test]
    fn exported_invoices_keep_the_settings_they_were_made_with() {
        let mut d = draft();
        d.frozen = Some(settings());
        let mut changed = settings();
        changed.tax_rate = "7".into();

        let invoice = build_invoice(&d, &changed, "RE-1").unwrap();

        assert_eq!(totals(&invoice).vat, "188.58");
    }

    #[test]
    fn a_half_filled_draft_can_still_be_previewed() {
        let mut d = draft();
        d.positions[0].price = String::new();
        d.positions[0].quantity = Some("x".into());
        d.positions[1].description = String::new();
        d.issue_date = "17.10".into();
        d.delivery_date = String::new();
        assert!(build_invoice(&d, &settings(), "X").is_err());

        let invoice = build_invoice(&for_preview(&d), &settings(), "X").unwrap();

        assert_eq!(invoice.lines[0].net_amount.to_string(), "0.00");
        assert_eq!(invoice.lines[1].item.name.as_deref(), Some("–"));
        assert_eq!(invoice.lines[0].quantity.to_string(), "1");
    }

    #[test]
    fn a_service_period_is_the_invoicing_period_instead_of_a_date() {
        let mut d = draft();
        d.delivery_date = "2026-09-01".into();
        d.delivery_end = Some("2026-09-30".into());

        let invoice = build_invoice(&d, &settings(), "RE-1").unwrap();

        let period = invoice.invoicing_period.unwrap();
        assert_eq!(period.start, Date::parse("2026-09-01").ok());
        assert_eq!(period.end, Date::parse("2026-09-30").ok());
        assert!(invoice.delivery.and_then(|d| d.date).is_none());
    }

    #[test]
    fn a_service_period_needs_an_end_on_or_after_its_start() {
        let refused = |end: &str| {
            let mut d = draft();
            d.delivery_date = "2026-09-10".into();
            d.delivery_end = Some(end.into());
            build_invoice(&d, &settings(), "X")
                .err()
                .is_some_and(|problems| problems.iter().any(|p| p.field == "delivery_end"))
        };

        assert!(refused(""));
        assert!(refused("2026-09-09"));
        assert!(!refused("2026-09-10"));

        // the preview shows the half typed period as one day
        let mut d = draft();
        d.delivery_end = Some(String::new());
        assert!(build_invoice(&for_preview(&d), &settings(), "X").is_ok());
    }

    #[test]
    fn the_seller_fields_for_layouts_are_not_in_the_xml() {
        let d = draft();
        let plain = build_invoice(&d, &settings(), "RE-1").unwrap();
        let mut more = settings();
        more.seller.mobile = Some("0171 555".into());
        more.seller.fax = Some("+49 221 123457".into());
        more.seller.website = Some("https://muster.example".into());
        more.seller.bank_name = Some("Musterbank".into());
        let full = build_invoice(&d, &more, "RE-1").unwrap();

        let xml = |invoice| crate::invoice::to_cii(invoice, d.profile).unwrap();

        assert_eq!(xml(plain), xml(full));
    }

    #[test]
    fn a_customer_without_an_address_writes_no_empty_elements() {
        let mut d = draft();
        d.customer.address = Address::default();

        let invoice = build_invoice(&d, &settings(), "RE-1").unwrap();
        let xml = crate::invoice::to_cii(invoice, d.profile).unwrap();

        // an opening tag right before its closing one, `<ram:CityName></ram:CityName>`
        let empty = xml.match_indices("></").any(|(at, _)| {
            let open = xml[..at].rfind('<').unwrap();
            !xml[open..].starts_with("</")
        });
        assert!(!empty, "{xml}");
    }

    #[test]
    fn a_customer_address_is_needed_above_250_euros_gross_only() {
        let severity = |price: &str, address: Address| {
            let mut d = draft();
            d.customer.address = address;
            d.positions = vec![Position {
                description: "Beratung".into(),
                quantity: None,
                unit: None,
                unit_label: None,
                price: price.into(),
            }];
            let invoice = build_invoice(&d, &settings(), "RE-1").unwrap();
            crate::invoice::validate(&invoice, ProfileKind::En16931)
                .into_iter()
                .find(|f| f.rule == "§ 14 UStG")
                .map(|f| f.severity)
        };
        let none = Address::default;

        // 19 %: 210.08 net is 250.00 gross, a Kleinbetragsrechnung; one cent more is not
        assert_eq!(severity("210.08", none()), Some(en16931::Severity::Warning));
        assert_eq!(severity("210.09", none()), Some(en16931::Severity::Fatal));
        assert_eq!(severity("1000", draft().customer.address), None);
    }

    #[test]
    fn a_draft_without_positions_is_refused() {
        let mut d = draft();
        d.positions.clear();

        assert!(build_invoice(&d, &settings(), "X")
            .unwrap_err()
            .iter()
            .any(|p| p.field == "positions"));
    }

    #[test]
    fn a_tax_number_alone_identifies_the_seller() {
        let mut only_tax = settings();
        only_tax.seller.vat_id = None;
        only_tax.seller.tax_number = Some("123/456/78901".into());

        for profile in [ProfileKind::En16931, ProfileKind::XRechnung] {
            let mut d = draft();
            d.profile = profile;
            d.buyer_reference = Some("991-01234-56".into());
            let invoice = build_invoice(&d, &only_tax, "RE-1").unwrap();

            assert_eq!(
                invoice.seller.identifiers,
                [Identifier::new("123/456/78901")]
            );
            assert!(validate(&invoice, profile).is_empty(), "{profile:?}");
        }
    }

    #[test]
    fn a_vat_id_is_the_identifier_and_no_tax_number_is_repeated() {
        let mut both = settings();
        both.seller.tax_number = Some("123/456/78901".into());

        let invoice = build_invoice(&draft(), &both, "RE-1").unwrap();

        assert_eq!(invoice.seller.identifiers, Vec::new());
        assert_eq!(
            invoice.seller.vat_identifier.as_deref(),
            Some("DE123456789")
        );
    }
}
