//! The invoice model and its CII serialization.
//!
//! The model is the `en16931` crate's `Invoice` (EN 16931 semantic model, serde-ready), so there is
//! no second, hand-written mirror to keep in sync. This module only adds the choice of profile, a
//! flat list of findings for the UI and the CII writer behind a proof of validity.
//!
//! These checks give early feedback. The authority over a finished file is the validator that runs
//! on the combined PDF (see the validation gate in the plan).

use en16931::profiles::{self, En16931, XRechnung};
use en16931::validation::profile::Validated;
use en16931_formats::Syntax;
use serde::{Deserialize, Serialize};

use en16931::invoice::PriceDetails;
pub use en16931::validation::Finding;
pub use en16931::Invoice;
use en16931::{AmountError, InvoiceAmount, InvoiceLine, Percentage, Quantity, UnitPriceAmount};
use rust_decimal::RoundingStrategy;

mod german;

pub use german::localize_issue;
#[cfg(test)]
pub(crate) mod golden;

/// What the invoice has to conform to.
///
/// EN 16931 is the default (a plain ZUGFeRD/Factur-X invoice). `XRechnung` adds the German
/// public-sector rules (`BR-DE-*`), e.g. a buyer reference (Leitweg-ID) and a seller contact.
/// MINIMUM and BASIC WL are deliberately absent: they are not valid e-invoices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileKind {
    #[default]
    En16931,
    #[serde(rename = "xrechnung")]
    XRechnung,
}

impl ProfileKind {
    pub(crate) fn profile(self) -> &'static en16931::Profile {
        match self {
            Self::En16931 => &profiles::EN16931,
            Self::XRechnung => &profiles::XRECHNUNG,
        }
    }
}

impl ProfileKind {
    pub fn specification_id(self) -> &'static str {
        self.profile().specification_id
    }
}

/// Everything the profile's rules have to say about the invoice, worst first as the crate orders it.
pub fn validate(invoice: &Invoice, profile: ProfileKind) -> Vec<Finding> {
    profile
        .profile()
        .validate(invoice)
        .findings()
        .iter()
        .cloned()
        .map(german::localize)
        .chain(recipient_address(invoice))
        .collect()
}

/// Up to this gross total an invoice may leave out the customer's address (Kleinbetragsrechnung,
/// § 33 `UStDV`: the Gesamtbetrag, VAT included)
const SMALL_INVOICE_LIMIT: rust_decimal::Decimal =
    rust_decimal::Decimal::from_parts(250, 0, 0, false, 0);

/// What German VAT law asks beyond EN 16931: the customer's full address (§ 14 Abs. 4 Nr. 1 `UStG`).
/// Above 250 € gross it stops the invoice, up to that it is a warning.
pub fn recipient_address(invoice: &Invoice) -> Option<Finding> {
    let address = &invoice.buyer.address;
    let complete = [&address.line1, &address.post_code, &address.city]
        .iter()
        .all(|part| part.as_deref().is_some_and(|p| !p.trim().is_empty()));
    if complete {
        return None;
    }
    let small = rust_decimal::Decimal::from(invoice.totals.gross_total) <= SMALL_INVOICE_LIMIT;
    Some(Finding {
        rule: "§ 14 UStG".into(),
        severity: if small {
            en16931::Severity::Warning
        } else {
            en16931::Severity::Fatal
        },
        path: en16931::Path::group(en16931::Group::Buyer),
        message: if small {
            "Die Anschrift des Kunden ist unvollständig. Bis 250 € brutto ist das als \
             Kleinbetragsrechnung erlaubt, darüber braucht die Rechnung Straße, PLZ und Ort."
        } else {
            "Über 250 € brutto braucht die Rechnung die vollständige Anschrift des Kunden: \
             Straße, PLZ und Ort."
        }
        .into(),
        detail: None,
        hint: Some("§ 14 Abs. 4 Nr. 1 UStG, § 33 UStDV".into()),
    })
}

/// Serializes the invoice as UN/CEFACT CII (`factur-x.xml`, or the `XRechnung` file).
///
/// Only an invoice that passes the profile's rules is written, BT-24 is stamped from the profile.
pub fn to_cii(invoice: Invoice, profile: ProfileKind) -> Result<String, Vec<Finding>> {
    let rejected = |r: en16931::validation::profile::Rejected| {
        r.report()
            .findings()
            .iter()
            .cloned()
            .map(german::localize)
            .collect::<Vec<_>>()
    };
    Ok(match profile {
        ProfileKind::En16931 => {
            let proof = Validated::<En16931>::new(invoice).map_err(rejected)?;
            en16931_formats::write(&proof, Syntax::Cii).xml
        }
        ProfileKind::XRechnung => {
            let proof = Validated::<XRechnung>::new(invoice).map_err(rejected)?;
            en16931_formats::write(&proof, Syntax::Cii).xml
        }
    })
}

/// A line from the price per unit, the way a form has it.
///
/// `InvoiceLine::new` takes the line's *net amount* and derives the unit price from it, which
/// shows up as `4,1666…` on the page for 3 × 12.50. Here the unit price is kept as entered and the
/// net amount (BT-131) is quantity × price rounded to cents. Allowances and charges of the line
/// must be added to the net amount by the caller, the model does not do it.
///
/// # Errors
///
/// When the product does not fit an invoice amount.
#[cfg_attr(not(test), allow(dead_code))] // used by the invoice form, see the plan
pub fn line_from_unit_price(
    id: &str,
    name: &str,
    quantity: Quantity,
    unit_code: &str,
    unit_price: UnitPriceAmount,
    category: &str,
    rate: Option<Percentage>,
) -> Result<InvoiceLine, AmountError> {
    let net = (quantity.into_decimal() * unit_price.into_decimal())
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let line = InvoiceLine::new(
        id,
        name,
        quantity,
        unit_code,
        InvoiceAmount::from_decimal_exact(net)?,
        category,
        rate,
    );
    Ok(line.with_price(PriceDetails {
        net_price: unit_price,
        ..PriceDetails::default()
    }))
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    fn line(quantity: rust_decimal::Decimal, price: rust_decimal::Decimal) -> InvoiceLine {
        line_from_unit_price(
            "1",
            "Beratung",
            Quantity::new(quantity),
            "HUR",
            UnitPriceAmount::new(price),
            "S",
            Some(Percentage::new(dec!(19))),
        )
        .unwrap()
    }

    #[test]
    fn the_unit_price_is_kept_and_the_net_amount_is_rounded_to_cents() {
        let line = line(dec!(3), dec!(12.50));
        assert_eq!(line.price.net_price.to_string(), "12.5");
        assert_eq!(line.net_amount.to_string(), "37.50");

        let rounded = line_from_unit_price(
            "2",
            "Material",
            Quantity::new(dec!(3)),
            "C62",
            UnitPriceAmount::new(dec!(0.33335)),
            "S",
            None,
        )
        .unwrap();
        assert_eq!(rounded.price.net_price.to_string(), "0.33335");
        assert_eq!(rounded.net_amount.to_string(), "1.00");
    }

    #[test]
    fn half_cents_round_away_from_zero() {
        assert_eq!(line(dec!(1), dec!(0.125)).net_amount.to_string(), "0.13");
        assert_eq!(line(dec!(2), dec!(0.125)).net_amount.to_string(), "0.25");
    }
}
