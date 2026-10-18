//! The invoice as the template wants it: strings only, German formatting, one place for the
//! presentation rules (unit names, address lines, which totals to show). Money is never computed
//! here, every figure comes from the reconciled invoice.

use en16931::invoice::{DocumentKind, PaymentMeans, PostalAddress};
use en16931::{Date, Invoice};
use serde::{Deserialize, Serialize};

use crate::model::{CustomLayout, Insets, InvoiceDraft, Settings};

/// What the user adds to the plain invoice data: look and feel of the document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Branding {
    /// A PNG, JPEG or SVG shown in the page header instead of the seller name.
    pub logo: Option<std::path::PathBuf>,
    /// Free text for the page footer, e.g. managing directors or the commercial register entry.
    pub footer: Option<String>,
    /// The texts around the table: salutation and introduction, closing
    pub intro: Option<String>,
    pub closing: Option<String>,
    /// The user's own layout when one was chosen: stationery, margins and font
    pub custom: Option<CustomLayout>,
    /// Draws lines where the margins are, for looking at a layout while it is made
    pub guides: bool,
}

impl Branding {
    /// Look, feel and letter texts of an invoice: what it chose for itself, else the settings.
    pub fn for_draft(draft: &InvoiceDraft, settings: &Settings, customs: &[CustomLayout]) -> Self {
        // what the invoice has is its own, even when it is empty (no text for this one); only an
        // invoice that never had a text (made before the texts existed) takes the settings
        let pick = |own: &Option<String>, default: &Option<String>| {
            own.as_ref()
                .or(default.as_ref())
                .filter(|text| !text.trim().is_empty())
                .cloned()
        };
        let wanted = draft
            .layout
            .clone()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| settings.layout.clone());
        // an exported invoice keeps the layout as it was, whatever became of the original
        let custom = draft
            .frozen_layout
            .clone()
            .or_else(|| customs.iter().find(|l| l.key() == wanted).cloned());
        Self {
            logo: settings.logo.clone(),
            footer: settings.footer.clone(),
            custom,
            guides: false,
            intro: pick(&draft.intro, &settings.intro),
            closing: pick(&draft.closing, &settings.closing),
        }
    }
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
    /// The letter around the table
    pub letter: Letter,
    /// Single facts for layouts that arrange them their own way
    pub facts: Facts,
    /// Account holder, IBAN and BIC of the credit transfer, empty without one
    pub bank: Vec<Labeled>,
    /// Paper, margins and font: what an own layout changes
    pub page: PageView,
}

/// Millimetres, files are names next to the template.
#[derive(Debug, Default, Serialize)]
pub struct PageView {
    pub font: Option<String>,
    /// Content insets for all pages, `None` keeps those of the layout
    pub margin: Option<Insets>,
    /// Insets of the first page when it differs from the others, e.g. for a letterhead
    pub first_margin: Option<Insets>,
    pub background: Option<BackgroundView>,
    /// Lines where the margins are
    pub guides: bool,
}

#[derive(Debug, Serialize)]
pub struct BackgroundView {
    pub first: String,
    pub rest: Option<String>,
}

impl PageView {
    /// `files` are the names the stationery got next to the template (first and following page).
    pub fn of(custom: Option<&CustomLayout>, files: Option<(String, Option<String>)>) -> Self {
        let Some(custom) = custom else {
            return Self::default();
        };
        Self {
            font: custom.fonts.first().map(|f| f.family.clone()),
            margin: Some(custom.margins),
            first_margin: custom
                .first_margins
                .filter(|first| *first != custom.margins),
            background: files.map(|(first, rest)| BackgroundView { first, rest }),
            guides: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Letter {
    pub intro: Option<String>,
    pub closing: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Facts {
    pub number: String,
    pub date: String,
    pub service_date: Option<String>,
    /// The invoice date is also the date of service
    pub same_day: bool,
    pub due_date: Option<String>,
    pub buyer_reference: Option<String>,
    pub tax_number: Option<String>,
    pub vat_id: Option<String>,
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

fn bank_lines(invoice: &Invoice) -> Vec<Labeled> {
    let Some(PaymentMeans::CreditTransfer(transfers)) =
        invoice.payment.as_ref().and_then(|p| p.means.as_ref())
    else {
        return Vec::new();
    };
    transfers
        .iter()
        .flat_map(|t| {
            [
                ("Kontoinhaber", t.account_name.clone()),
                ("IBAN", t.account_identifier.clone()),
                ("BIC", t.provider_identifier.clone()),
            ]
        })
        .filter_map(|(label, value)| {
            value.map(|value| Labeled {
                label: label.into(),
                value,
            })
        })
        .collect()
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

pub fn build(
    invoice: &Invoice,
    branding: &Branding,
    logo: Option<String>,
    page: PageView,
) -> InvoiceView {
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
        letter: Letter {
            intro: branding.intro.clone(),
            closing: branding.closing.clone(),
        },
        facts: Facts {
            number: invoice.number.clone().unwrap_or_default(),
            date: de_date(issue),
            service_date: invoice.delivery.as_ref().and_then(|d| d.date).map(de_date),
            same_day: invoice.delivery.as_ref().and_then(|d| d.date) == Some(issue),
            due_date: invoice.due_date.map(de_date),
            buyer_reference: invoice.buyer_reference.clone(),
            tax_number: invoice.seller.tax_registration.clone(),
            vat_id: invoice.seller.vat_identifier.clone(),
        },
        bank: bank_lines(invoice),
        page,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_layout(id: i64) -> CustomLayout {
        CustomLayout {
            id: Some(id),
            name: "Briefpapier".into(),
            background: None,
            margins: Insets::default(),
            first_margins: None,
            fonts: Vec::new(),
            archived: false,
        }
    }

    #[test]
    fn an_own_layout_is_found_by_its_key() {
        let settings = Settings {
            layout: "custom-3".into(),
            ..Settings::default()
        };
        let draft = InvoiceDraft::blank("2026-10-18");

        let found = Branding::for_draft(&draft, &settings, &[own_layout(2), own_layout(3)]);
        assert_eq!(found.custom.and_then(|c| c.id), Some(3));

        // gone: the standard layout takes over, nothing breaks
        let gone = Branding::for_draft(&draft, &settings, &[own_layout(2)]);
        assert!(gone.custom.is_none());
    }

    #[test]
    fn an_exported_invoice_keeps_the_own_layout_it_was_made_with() {
        let settings = Settings {
            layout: "custom-3".into(),
            ..Settings::default()
        };
        let mut draft = InvoiceDraft::blank("2026-10-18");
        let mut frozen = own_layout(3);
        frozen.name = "As exported".into();
        draft.frozen_layout = Some(frozen);

        let branding = Branding::for_draft(&draft, &settings, &[own_layout(3)]);
        assert_eq!(
            branding.custom.map(|c| c.name).as_deref(),
            Some("As exported")
        );
    }

    #[test]
    fn the_first_page_has_its_own_margins_only_when_they_differ() {
        let mut layout = own_layout(1);
        layout.margins = Insets {
            top: 20.0,
            right: 20.0,
            bottom: 25.0,
            left: 25.0,
        };

        layout.first_margins = Some(layout.margins);
        assert!(PageView::of(Some(&layout), None).first_margin.is_none());

        let own = Insets {
            top: 60.0,
            right: 10.0,
            bottom: 10.0,
            left: 30.0,
        };
        layout.first_margins = Some(own);
        assert_eq!(PageView::of(Some(&layout), None).first_margin, Some(own));
        assert!(PageView::of(None, None).margin.is_none());
    }

    #[test]
    fn an_invoice_own_layout_and_texts_win_over_the_settings() {
        let settings = Settings {
            layout: "custom-3".into(),
            intro: Some("Aus den Einstellungen".into()),
            closing: Some("Danke".into()),
            ..Settings::default()
        };
        let plain = InvoiceDraft::blank("2026-10-17");
        let own = InvoiceDraft {
            layout: Some("standard".into()),
            intro: Some("Nur für diese Rechnung".into()),
            closing: Some("   ".into()),
            ..plain.clone()
        };

        let customs = [own_layout(3)];
        let by_settings = Branding::for_draft(&plain, &settings, &customs);
        let by_invoice = Branding::for_draft(&own, &settings, &customs);

        assert!(by_settings.custom.is_some());
        assert_eq!(by_settings.intro.as_deref(), Some("Aus den Einstellungen"));
        // the invoice asked for the built-in layout
        assert!(by_invoice.custom.is_none());
        assert_eq!(by_invoice.intro.as_deref(), Some("Nur für diese Rechnung"));
        // an empty text is the invoice's own: no closing, whatever the settings say
        assert_eq!(by_invoice.closing, None);
        assert_eq!(by_settings.closing.as_deref(), Some("Danke"));
    }

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

        let view = build(&invoice, &Branding::default(), None, PageView::default());

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
