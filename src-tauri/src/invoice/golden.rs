//! Golden invoices: the cases the plan names, built through the public builder, each one must pass
//! its profile and serialize. They double as the input set for the external validators later.

use en16931::invoice::{
    Code, Delivery, DocumentAllowanceCharge, LineAllowanceCharge, LineVat, Party, PartyRole,
    PaymentInstructions, PostalAddress,
};
use en16931::reconcile::Reconciler;
use en16931::{
    Date, Identifier, InvoiceAmount, InvoiceLine, Percentage, Quantity, UnitPriceAmount,
};
use rust_decimal_macros::dec;

use super::{to_cii, validate, Invoice, ProfileKind};

fn amount(s: &str) -> InvoiceAmount {
    InvoiceAmount::parse(s).expect("a two-decimal amount")
}

fn line(
    id: &str,
    name: &str,
    qty: i64,
    unit_price: &str,
    category: &str,
    rate: i64,
) -> InvoiceLine {
    super::line_from_unit_price(
        id,
        name,
        Quantity::new(rust_decimal::Decimal::from(qty)),
        "C62",
        UnitPriceAmount::new(unit_price.parse().expect("a decimal price")),
        category,
        Some(Percentage::new(rust_decimal::Decimal::from(rate))),
    )
    .expect("the line fits an invoice amount")
}

fn seller() -> Party {
    Party::new(
        "Muster Software GmbH",
        PostalAddress::new("Hauptstraße 5", "Köln", "50667", "DE").unwrap(),
    )
    .with_vat_identifier("DE123456789")
    .with_electronic_address(Identifier::eas("rechnung@muster.example", "EM").unwrap())
    .with_contact(en16931::invoice::Contact::new(
        "Max Muster",
        "+49 221 123456",
        "max@muster.example",
    ))
}

fn buyer() -> Party {
    Party::new(
        "Beispiel AG",
        PostalAddress::new("Marktplatz 2", "Beispielstadt", "54321", "DE").unwrap(),
    )
    .with_electronic_address(Identifier::eas("einkauf@beispiel.example", "EM").unwrap())
}

fn builder_for(number: &str, profile: ProfileKind) -> en16931::invoice::InvoiceBuilder {
    Invoice::builder(
        profile.profile().specification_id,
        number,
        Date::new(2026, 10, 17).unwrap(),
        "380",
        "EUR",
    )
    .seller(seller())
    .buyer(buyer())
    .due_in_days(14)
    // § 14 UStG wants the service date, and CII otherwise leaves its mandatory delivery element
    // empty, which the Peppol rule R008 flags
    .delivery(Delivery {
        date: Some(Date::new(2026, 10, 17).unwrap()),
        ..Delivery::default()
    })
    .payment(PaymentInstructions::sepa_credit_transfer(
        "DE02120300000000202051",
        Some("BYLADEM1001"),
    ))
}

fn builder(number: &str) -> en16931::invoice::InvoiceBuilder {
    builder_for(number, ProfileKind::En16931)
}

/// Checks the invoice against its profile and serializes it. With `GOLDEN_OUT` set to a directory
/// the XML is written there as `<name>.<profile>.xml`, for the external validators.
fn assert_valid(name: &str, invoice: &Invoice, profile: ProfileKind) {
    let findings = validate(invoice, profile);
    assert!(findings.is_empty(), "{name}: {findings:#?}");
    let xml = to_cii(invoice.clone(), profile).expect("a valid invoice serializes");
    assert!(xml.contains("<rsm:CrossIndustryInvoice"));
    assert!(xml.contains(profile.profile().specification_id));

    if let Some(dir) = std::env::var_os("GOLDEN_OUT") {
        let kind = serde_json::to_value(profile).unwrap();
        let file =
            std::path::Path::new(&dir).join(format!("{name}.{}.xml", kind.as_str().unwrap()));
        std::fs::write(file, xml).unwrap();
    }
}

fn standard() -> Invoice {
    builder("RE-1")
        .line(line("1", "Beratung", 10, "95.00", "S", 19))
        .build_reconciled()
        .unwrap()
}

fn multi_rate() -> Invoice {
    builder("RE-2")
        .line(line("1", "Software", 1, "1000.00", "S", 19))
        .line(line("2", "Fachbuch", 2, "24.90", "S", 7))
        .build_reconciled()
        .unwrap()
}

fn allowances_charges() -> Invoice {
    let mut discounted = line("1", "Schulung", 1, "1000.00", "S", 19);
    // BT-131 is the line total *after* its allowances and charges, the reconciler takes it as is
    discounted.net_amount = amount("900.00");
    discounted.allowances.push(LineAllowanceCharge {
        amount: amount("100.00"),
        base_amount: None,
        percentage: None,
        reason: Some("Rabatt".into()),
        reason_code: None,
    });
    let vat = LineVat {
        category: Code::new("S"),
        rate: Some(Percentage::new(dec!(19))),
    };
    builder("RE-3")
        .line(discounted)
        .allowance(DocumentAllowanceCharge {
            amount: amount("50.00"),
            base_amount: None,
            percentage: None,
            vat: vat.clone(),
            reason: Some("Treuebonus".into()),
            reason_code: None,
        })
        .charge(DocumentAllowanceCharge {
            amount: amount("12.00"),
            base_amount: None,
            percentage: None,
            vat,
            reason: Some("Versand".into()),
            reason_code: None,
        })
        .build_reconciled()
        .unwrap()
}

fn many_lines() -> Invoice {
    (1..=70)
        .fold(builder("RE-8"), |builder, i| {
            builder.line(line(
                &i.to_string(),
                &format!("Position {i} mit Umlauten äöüß und € – Sonderzeichen łőñ"),
                i,
                "12.50",
                "S",
                19,
            ))
        })
        .build_reconciled()
        .unwrap()
}

fn credit_note() -> Invoice {
    standard().to_credit_note("GS-1", Date::new(2026, 10, 20).unwrap())
}

fn reverse_charge() -> Invoice {
    builder("RE-5")
        .buyer(buyer().with_vat_identifier("DE987654321"))
        .line(line("1", "Bauleistung", 1, "500.00", "AE", 0))
        .build_reconciled_with(&Reconciler::new().exemption("AE", None, Some("VATEX-EU-AE")))
        .unwrap()
}

fn xrechnung() -> Invoice {
    builder_for("RE-6", ProfileKind::XRechnung)
        .buyer_reference("991-01234-56")
        .business_process("urn:fdc:peppol.eu:2017:poacc:billing:01:1.0")
        .line(line("1", "Beratung", 1, "100.00", "S", 19))
        .build_reconciled()
        .unwrap()
}

/// Every valid golden invoice with the profile it is valid for.
pub(crate) fn cases() -> Vec<(&'static str, Invoice, ProfileKind)> {
    vec![
        ("standard", standard(), ProfileKind::En16931),
        ("multi-rate", multi_rate(), ProfileKind::En16931),
        (
            "allowances-charges",
            allowances_charges(),
            ProfileKind::En16931,
        ),
        ("many-lines", many_lines(), ProfileKind::En16931),
        ("credit-note", credit_note(), ProfileKind::En16931),
        ("reverse-charge", reverse_charge(), ProfileKind::En16931),
        ("xrechnung", xrechnung(), ProfileKind::XRechnung),
    ]
}

/// A valid invoice for the profile, the input of the pipeline tests that run the real tools.
pub(crate) fn standard_invoice(profile: ProfileKind) -> Invoice {
    match profile {
        ProfileKind::En16931 => standard(),
        ProfileKind::XRechnung => xrechnung(),
    }
}

#[test]
fn every_golden_case_passes_its_profile_and_serializes() {
    for (name, invoice, profile) in cases() {
        assert_valid(name, &invoice, profile);
    }
}

#[test]
fn multiple_tax_rates_form_one_breakdown_each() {
    assert_eq!(standard().vat_breakdown.len(), 1);
    assert_eq!(multi_rate().vat_breakdown.len(), 2);
}

#[test]
fn line_and_document_adjustments_end_up_in_the_totals() {
    // 1000 - 100 (line) - 50 + 12 (document)
    assert_eq!(
        allowances_charges().totals.taxable_total.to_string(),
        "862.00"
    );
}

#[test]
fn credit_note_points_back_at_the_invoice() {
    assert_eq!(credit_note().preceding_invoices.len(), 1);
}

#[test]
fn reverse_charge_has_no_vat() {
    assert_eq!(reverse_charge().totals.vat_total, Some(amount("0.00")));
}

#[test]
fn xrechnung_asks_for_buyer_reference_and_seller_contact() {
    let incomplete = builder_for("RE-6", ProfileKind::XRechnung)
        .line(line("1", "Beratung", 1, "100.00", "S", 19))
        .build_reconciled()
        .unwrap();
    let findings = validate(&incomplete, ProfileKind::XRechnung);

    assert!(
        findings.iter().any(|f| f.rule == "BR-DE-15"),
        "{findings:#?}"
    );
    assert!(to_cii(incomplete, ProfileKind::XRechnung).is_err());
}

#[test]
fn what_xrechnung_wants_of_a_buyer_is_known_up_front() {
    let gaps = Party::default().missing_for(&en16931::profiles::XRECHNUNG, PartyRole::Buyer);
    assert!(!gaps.is_empty());
}

#[test]
fn a_broken_invoice_is_never_written() {
    let wrong_total = builder("RE-7")
        .line(line("1", "Beratung", 1, "100.00", "S", 19))
        .build();

    assert!(!validate(&wrong_total, ProfileKind::En16931).is_empty());
    assert!(to_cii(wrong_total, ProfileKind::En16931).is_err());
}
