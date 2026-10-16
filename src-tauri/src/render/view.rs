//! The invoice as the template wants it: strings only, German formatting, one place for the
//! presentation rules (unit names, address lines, which totals to show). Money is never computed
//! here, every figure comes from the reconciled invoice.

use en16931::invoice::{DocumentKind, PaymentMeans, PostalAddress};
use en16931::{Date, Invoice};
use serde::{Deserialize, Serialize};

/// What the user adds to the plain invoice data: look and feel of the document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Branding {
    /// A PNG, JPEG or SVG shown in the page header instead of the seller name.
    pub logo: Option<std::path::PathBuf>,
    /// Free text for the page footer, e.g. managing directors or the commercial register entry.
    pub footer: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceView {
    pub title: &'static str,
    pub number: String,
    pub issue: YearMonthDay,
    pub seller: PartyView,
    pub buyer: PartyView,
    pub meta: Vec<Labeled>,
    pub intro: Vec<String>,
    pub lines: Vec<LineView>,
    pub totals: Vec<Total>,
    pub vat_notes: Vec<String>,
    pub payment: Vec<String>,
    pub footer: Option<String>,
    /// File name next to the template, set when a logo was copied there.
    pub logo: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct YearMonthDay {
    year: i32,
    month: u8,
    day: u8,
}

#[derive(Debug, Serialize)]
pub struct PartyView {
    pub name: String,
    pub address: Vec<String>,
    pub contact: Vec<String>,
    pub registration: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Labeled {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct LineView {
    pub pos: String,
    pub name: String,
    pub description: Option<String>,
    pub adjustments: Vec<String>,
    pub quantity: String,
    pub unit: String,
    pub unit_price: String,
    pub vat_rate: String,
    pub net: String,
}

#[derive(Debug, Serialize)]
pub struct Total {
    pub label: String,
    pub value: String,
    pub strong: bool,
}

/// `1234.5` → `1.234,5`, the sign stays in front.
pub(crate) fn de_number(plain: &str) -> String {
    let (sign, digits) = plain
        .strip_prefix('-')
        .map_or(("", plain), |rest| ("-", rest));
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(c);
    }
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped},{frac}")
    }
}

/// A quantity or rate without trailing zeros: `10.000` → `10`, `2.50` → `2,5`.
fn de_trimmed(plain: &str) -> String {
    let trimmed = if plain.contains('.') {
        plain.trim_end_matches('0').trim_end_matches('.')
    } else {
        plain
    };
    de_number(trimmed)
}

fn de_date(date: Date) -> String {
    format!("{:02}.{:02}.{}", date.day(), date.month(), date.year())
}

/// `5` → `5.00`, `0.5` → `0.50`: prices show cents even where the model drops trailing zeros.
fn two_decimals(plain: &str) -> String {
    let (int, frac) = plain.split_once('.').unwrap_or((plain, ""));
    format!("{int}.{frac:0<2}")
}

fn money(amount: impl std::fmt::Display, currency: &str) -> String {
    let symbol = if currency == "EUR" { "€" } else { currency };
    format!("{} {symbol}", de_number(&two_decimals(&amount.to_string())))
}

fn unit_name(code: &str) -> String {
    match code {
        "C62" | "H87" => "Stk.",
        "HUR" => "Std.",
        "DAY" => "Tag(e)",
        "MON" => "Monat(e)",
        "WEE" => "Woche(n)",
        "KGM" => "kg",
        "LTR" => "l",
        "MTR" => "m",
        "KWH" => "kWh",
        "LS" => "pauschal",
        other => other,
    }
    .to_owned()
}

/// The statement an invoice needs when it carries no VAT, for breakdowns that only name a category
/// or an exemption code. A reason text on the invoice always wins over these.
fn exemption_note(category: &str) -> Option<&'static str> {
    match category {
        "AE" => Some("Steuerschuldnerschaft des Leistungsempfängers (Reverse Charge, § 13b UStG)"),
        "K" => Some("Steuerfreie innergemeinschaftliche Lieferung"),
        "G" => Some("Steuerfreie Ausfuhrlieferung"),
        "E" => Some("Steuerbefreite Leistung"),
        "O" => Some("Nicht steuerbarer Umsatz"),
        _ => None,
    }
}

fn address_lines(address: &PostalAddress) -> Vec<String> {
    let town = [address.post_code.as_deref(), address.city.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    let country = address
        .country
        .as_ref()
        .map(|c| c.as_str().to_owned())
        .filter(|c| c != "DE");
    [&address.line1, &address.line2, &address.line3]
        .into_iter()
        .flatten()
        .cloned()
        .chain((!town.is_empty()).then_some(town))
        .chain(country)
        .collect()
}

fn party_view(party: &en16931::invoice::Party, seller: bool) -> PartyView {
    let contact = &party.contact;
    let contact_lines = [
        contact.name.clone(),
        contact.phone.as_ref().map(|p| format!("Tel. {p}")),
        contact.email.clone(),
    ];
    let registration = [
        party
            .vat_identifier
            .as_ref()
            .map(|v| format!("USt-IdNr. {v}")),
        party
            .tax_registration
            .as_ref()
            .filter(|_| seller)
            .map(|t| format!("Steuernr. {t}")),
        party.additional_legal_information.clone(),
    ];
    PartyView {
        name: party.name.clone().unwrap_or_default(),
        address: address_lines(&party.address),
        contact: contact_lines.into_iter().flatten().collect(),
        registration: registration.into_iter().flatten().collect(),
    }
}

fn payment_lines(invoice: &Invoice, currency: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(terms) = &invoice.payment_terms {
        out.push(terms.clone());
    }
    if let Some(due) = invoice.due_date {
        out.push(format!(
            "Zahlbar bis {}, Betrag {}",
            de_date(due),
            money(invoice.totals.due, currency)
        ));
    }
    if let Some(payment) = &invoice.payment {
        if let Some(PaymentMeans::CreditTransfer(transfers)) = &payment.means {
            for t in transfers {
                let parts = [
                    t.account_name.as_ref().map(|n| format!("Kontoinhaber {n}")),
                    t.account_identifier.as_ref().map(|i| format!("IBAN {i}")),
                    t.provider_identifier.as_ref().map(|b| format!("BIC {b}")),
                ];
                out.push(parts.into_iter().flatten().collect::<Vec<_>>().join(" · "));
            }
        }
        if let Some(reference) = &payment.remittance_information {
            out.push(format!("Verwendungszweck: {reference}"));
        }
    }
    out
}

pub fn build(invoice: &Invoice, branding: &Branding, logo: Option<String>) -> InvoiceView {
    let currency = invoice.currency.as_ref().map_or("EUR", |c| c.as_str());
    let totals = &invoice.totals;
    let credit = invoice.kind == DocumentKind::CreditNote;
    let issue = invoice
        .issue_date
        .unwrap_or_else(|| Date::new(1970, 1, 1).unwrap());

    let mut meta = vec![Labeled {
        label: if credit {
            "Gutschriftsnummer"
        } else {
            "Rechnungsnummer"
        }
        .into(),
        value: invoice.number.clone().unwrap_or_default(),
    }];
    meta.push(Labeled {
        label: "Datum".into(),
        value: de_date(issue),
    });
    if let Some(date) = invoice.delivery.as_ref().and_then(|d| d.date) {
        meta.push(Labeled {
            label: "Leistungsdatum".into(),
            value: de_date(date),
        });
    } else if let Some(period) = &invoice.invoicing_period {
        if let (Some(start), Some(end)) = (period.start, period.end) {
            meta.push(Labeled {
                label: "Leistungszeitraum".into(),
                value: format!("{} – {}", de_date(start), de_date(end)),
            });
        }
    }
    if let Some(due) = invoice.due_date {
        meta.push(Labeled {
            label: "Fällig am".into(),
            value: de_date(due),
        });
    }
    if let Some(reference) = &invoice.buyer_reference {
        meta.push(Labeled {
            label: "Käuferreferenz".into(),
            value: reference.clone(),
        });
    }
    if let Some(original) = invoice.preceding_invoices.first() {
        meta.push(Labeled {
            label: "Zu Rechnung".into(),
            value: original.reference.as_str().to_owned(),
        });
    }

    let lines = invoice
        .lines
        .iter()
        .map(|l| {
            let rate = l.vat.rate.map_or_else(
                || "USt. frei".to_owned(),
                |r| format!("{} % USt.", de_trimmed(&r.to_string())),
            );
            let adjustments = l
                .allowances
                .iter()
                .map(|a| (a, "Nachlass", "-"))
                .chain(l.charges.iter().map(|c| (c, "Zuschlag", "+")))
                .map(|(a, name, sign)| {
                    let reason = a.reason.as_deref().unwrap_or(name);
                    format!("{reason}: {sign}{}", money(a.amount, currency))
                })
                .collect();
            LineView {
                pos: l.id.clone(),
                name: l.item.name.clone().unwrap_or_default(),
                description: l.item.description.clone(),
                adjustments,
                quantity: de_trimmed(&l.quantity.to_string()),
                unit: unit_name(l.unit_code.as_str()),
                unit_price: money(l.price.net_price, currency),
                vat_rate: rate,
                net: money(l.net_amount, currency),
            }
        })
        .collect();

    let mut rows = vec![Total {
        label: "Summe der Positionen".into(),
        value: money(totals.line_total, currency),
        strong: false,
    }];
    if let Some(a) = totals.allowance_total.filter(|a| !a.is_zero()) {
        rows.push(Total {
            label: "Nachlässe".into(),
            value: format!("-{}", money(a, currency)),
            strong: false,
        });
    }
    if let Some(c) = totals.charge_total.filter(|c| !c.is_zero()) {
        rows.push(Total {
            label: "Zuschläge".into(),
            value: format!("+{}", money(c, currency)),
            strong: false,
        });
    }
    rows.push(Total {
        label: "Nettobetrag".into(),
        value: money(totals.taxable_total, currency),
        strong: false,
    });
    for entry in &invoice.vat_breakdown {
        let rate = entry
            .rate
            .map_or_else(|| "0".to_owned(), |r| de_trimmed(&r.to_string()));
        rows.push(Total {
            label: format!(
                "Umsatzsteuer {rate} % auf {}",
                money(entry.taxable_amount, currency)
            ),
            value: money(entry.tax_amount, currency),
            strong: false,
        });
    }
    rows.push(Total {
        label: "Gesamtbetrag".into(),
        value: money(totals.gross_total, currency),
        strong: true,
    });
    if let Some(paid) = totals.paid.filter(|p| !p.is_zero()) {
        rows.push(Total {
            label: "Bereits bezahlt".into(),
            value: format!("-{}", money(paid, currency)),
            strong: false,
        });
    }
    if let Some(rounding) = totals.rounding.filter(|r| !r.is_zero()) {
        rows.push(Total {
            label: "Rundung".into(),
            value: money(rounding, currency),
            strong: false,
        });
    }
    rows.push(Total {
        label: if credit {
            "Gutschriftsbetrag"
        } else {
            "Zahlbetrag"
        }
        .into(),
        value: money(totals.due, currency),
        strong: true,
    });

    let mut vat_notes: Vec<String> = invoice
        .vat_breakdown
        .iter()
        .filter_map(|e| {
            e.exemption_reason
                .clone()
                .or_else(|| exemption_note(e.category.as_str()).map(str::to_owned))
        })
        .collect();
    vat_notes.dedup();

    InvoiceView {
        title: if credit { "Gutschrift" } else { "Rechnung" },
        number: invoice.number.clone().unwrap_or_default(),
        issue: YearMonthDay {
            year: issue.year(),
            month: issue.month(),
            day: issue.day(),
        },
        seller: party_view(&invoice.seller, true),
        buyer: party_view(&invoice.buyer, false),
        meta,
        intro: invoice
            .notes
            .iter()
            .filter_map(|n| n.note.clone())
            .collect(),
        lines,
        totals: rows,
        vat_notes,
        payment: payment_lines(invoice, currency),
        footer: branding.footer.clone(),
        logo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_numbers_group_thousands_and_use_a_comma() {
        assert_eq!(de_number("1234567.89"), "1.234.567,89");
        assert_eq!(de_number("999.5"), "999,5");
        assert_eq!(de_number("-1000.00"), "-1.000,00");
        assert_eq!(de_number("12"), "12");
    }

    #[test]
    fn quantities_lose_trailing_zeros() {
        assert_eq!(de_trimmed("10.000"), "10");
        assert_eq!(de_trimmed("2.50"), "2,5");
        assert_eq!(de_trimmed("19"), "19");
    }

    #[test]
    fn a_reverse_charge_invoice_states_it() {
        let invoice = crate::invoice::golden::cases()
            .into_iter()
            .find(|(name, ..)| *name == "reverse-charge")
            .unwrap()
            .1;

        let view = build(&invoice, &Branding::default(), None);

        assert_eq!(view.vat_notes.len(), 1);
        assert!(view.vat_notes[0].contains("Steuerschuldnerschaft"));
    }

    #[test]
    fn vat_free_categories_get_their_statement() {
        assert!(exemption_note("AE").unwrap().contains("§ 13b"));
        assert!(exemption_note("K").is_some());
        assert_eq!(exemption_note("S"), None);
        assert_eq!(exemption_note("Z"), None);
    }

    #[test]
    fn money_always_shows_cents() {
        assert_eq!(money("1000", "EUR"), "1.000,00 €");
        assert_eq!(money("0.5", "EUR"), "0,50 €");
        assert_eq!(money("19.999", "EUR"), "19,999 €");
    }

    #[test]
    fn money_uses_the_euro_sign() {
        assert_eq!(money("1234.50", "EUR"), "1.234,50 €");
        assert_eq!(money("5.00", "CHF"), "5,00 CHF");
    }
}
