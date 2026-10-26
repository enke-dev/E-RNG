//! The invoice as the template wants it: strings only, German formatting, one place for the
//! presentation rules (unit names, address lines, which totals to show). Money is never computed
//! here, every figure comes from the reconciled invoice.

use en16931::invoice::{DocumentKind, PaymentMeans, PostalAddress};
use en16931::{Date, Invoice};
use serde::{Deserialize, Serialize};

use super::format::Formatter;
use super::parts::PartView;
use crate::model::{CustomLayout, Format, Insets, InvoiceDraft, Settings};

/// What the user adds to the plain invoice data: look and feel of the document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Branding {
    /// The legal information of the seller (managing directors, register entry), for `{legal}`
    pub legal: Option<String>,
    /// No VAT is charged: the document shows no rate, no VAT rows and no gross sum, only the reason
    pub vat_free: bool,
    /// The settings have no payment term: the document shows neither a due date nor payment terms
    pub no_payment_term: bool,
    /// The texts around the table: salutation and introduction, closing
    pub intro: Option<String>,
    pub closing: Option<String>,
    /// The user's own layout when one was chosen, else the built-in one is used
    pub custom: Option<CustomLayout>,
    /// Draws lines where the margins are and where a letter has its window and folds, for looking
    /// at a layout while it is made
    pub guides: bool,
    /// Marks what is wrong with a layout (a blurry image) in the sample, never in a real invoice
    pub marks: bool,
}

impl Branding {
    /// Look, feel and letter texts of an invoice: what it chose for itself, else the settings.
    pub fn for_draft(draft: &InvoiceDraft, settings: &Settings, customs: &[CustomLayout]) -> Self {
        // what the invoice has is its own, even when it is empty (no text for this one); only an
        // invoice that never had a text (made before the texts existed) takes the settings
        let pick = |own: Option<&str>, default: Option<&str>| {
            own.or(default)
                .filter(|text| !text.trim().is_empty())
                .map(str::to_owned)
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
            // what an invoice froze before the legal information was the seller's is its footer
            legal: settings
                .seller
                .legal_info
                .clone()
                .or_else(|| settings.footer.clone()),
            vat_free: settings.vat_free(),
            no_payment_term: settings.payment_days.is_none(),
            custom,
            guides: false,
            marks: false,
            intro: pick(draft.intro.as_deref(), settings.intro.as_deref()),
            closing: pick(draft.closing.as_deref(), settings.closing.as_deref()),
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
    /// Facts of the invoice by key, what a part of the layout picks its rows from
    pub infos: Vec<Info>,
    pub intro: Vec<String>,
    pub lines: Vec<LineView>,
    pub totals: Vec<Total>,
    pub vat_notes: Vec<String>,
    pub payment: Vec<String>,
    pub legal: Option<String>,
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
    /// The frame of the content on the pages after the first, and on the first unless it differs
    pub margin: Insets,
    /// The frame of the first page when it differs from the others, e.g. for a letterhead
    pub first_margin: Option<Insets>,
    /// What is placed on the pages after the first
    pub parts: Vec<PartView>,
    /// What is placed on the first page when it has parts of its own
    pub first_parts: Option<Vec<PartView>>,
    /// Lines where the margins are, and the aids of a letter
    pub guides: bool,
    /// Marks what is wrong, for the sample
    pub marks: bool,
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

/// A fact of the invoice a part can show.
#[derive(Debug, Serialize)]
pub struct Info {
    pub key: &'static str,
    pub label: String,
    pub value: String,
    /// Dates as `18. Oktober 2026`, the same as `value` for the rest
    pub value_long: String,
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
    ];
    PartyView {
        name: party.name.clone().unwrap_or_default(),
        address: address_lines(&party.address),
        contact: contact_lines.into_iter().flatten().collect(),
        registration: registration.into_iter().flatten().collect(),
    }
}

fn bank_lines(invoice: &Invoice, fmt: &Formatter) -> Vec<Labeled> {
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
                ("IBAN", t.account_identifier.as_deref().map(|i| fmt.iban(i))),
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

/// `with_term`: the payment terms and the due date are part of it, else only how to pay
fn payment_lines(
    invoice: &Invoice,
    currency: &str,
    fmt: &Formatter,
    with_term: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(terms) = invoice.payment_terms.as_ref().filter(|_| with_term) {
        out.push(terms.clone());
    }
    if let Some(due) = invoice.due_date.filter(|_| with_term) {
        out.push(format!(
            "Zahlbar bis {}, Betrag {}",
            fmt.date(due),
            fmt.money(invoice.totals.due, currency)
        ));
    }
    if let Some(payment) = &invoice.payment {
        if let Some(PaymentMeans::CreditTransfer(transfers)) = &payment.means {
            for t in transfers {
                let parts = [
                    t.account_name.as_ref().map(|n| format!("Kontoinhaber {n}")),
                    t.account_identifier
                        .as_ref()
                        .map(|i| format!("IBAN {}", fmt.iban(i))),
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

/// The facts about the document, as the infos part lists them.
fn info_rows(invoice: &Invoice, branding: &Branding, fmt: &Formatter, issue: Date) -> Vec<Info> {
    let credit = invoice.kind == DocumentKind::CreditNote;
    let date = |key: &'static str, label: &str, date: Date| Info {
        key,
        label: label.into(),
        value: fmt.date(date),
        value_long: fmt.date_long(date),
    };
    let text = |key: &'static str, label: &str, value: String| Info {
        key,
        label: label.into(),
        value_long: value.clone(),
        value,
    };
    let mut infos = vec![text(
        "number",
        if credit {
            "Gutschriftsnummer"
        } else {
            "Rechnungsnummer"
        },
        invoice.number.clone().unwrap_or_default(),
    )];
    infos.push(date("date", "Datum", issue));
    if let Some(day) = invoice.delivery.as_ref().and_then(|d| d.date) {
        infos.push(date("service-date", "Leistungsdatum", day));
    } else if let Some(period) = &invoice.invoicing_period {
        if let (Some(start), Some(end)) = (period.start, period.end) {
            infos.push(Info {
                key: "service-date",
                label: "Leistungszeitraum".into(),
                value: format!("{} – {}", fmt.date(start), fmt.date(end)),
                value_long: format!("{} – {}", fmt.date_long(start), fmt.date_long(end)),
            });
        }
    }
    if let Some(due) = invoice.due_date.filter(|_| !branding.no_payment_term) {
        infos.push(date("due-date", "Fällig am", due));
    }
    if let Some(reference) = &invoice.buyer_reference {
        infos.push(text("buyer-reference", "Käuferreferenz", reference.clone()));
    }
    if let Some(original) = invoice.preceding_invoices.first() {
        infos.push(text(
            "preceding-invoice",
            "Zu Rechnung",
            original.reference.as_str().to_owned(),
        ));
    }
    if let Some(number) = &invoice.seller.tax_registration {
        infos.push(text("tax-number", "Steuernummer", number.clone()));
    }
    if let Some(id) = &invoice.seller.vat_identifier {
        infos.push(text("vat-id", "USt-IdNr.", id.clone()));
    }
    infos
}

/// The positions of the table.
fn line_views(
    invoice: &Invoice,
    branding: &Branding,
    fmt: &Formatter,
    currency: &str,
) -> Vec<LineView> {
    invoice
        .lines
        .iter()
        .map(|l| {
            let rate = if branding.vat_free {
                String::new()
            } else {
                l.vat.rate.map_or_else(
                    || "USt. frei".to_owned(),
                    |r| format!("{} USt.", fmt.percent(&r.to_string())),
                )
            };
            let adjustments = l
                .allowances
                .iter()
                .map(|a| (a, "Nachlass", fmt.minus()))
                .chain(l.charges.iter().map(|c| (c, "Zuschlag", "+")))
                .map(|(a, name, sign)| {
                    let reason = a.reason.as_deref().unwrap_or(name);
                    format!("{reason}: {sign}{}", fmt.money(a.amount, currency))
                })
                .collect();
            LineView {
                pos: l.id.clone(),
                name: l.item.name.clone().unwrap_or_default(),
                description: l.item.description.clone(),
                adjustments,
                quantity: fmt.trimmed(&l.quantity.to_string()),
                unit: unit_name(l.unit_code.as_str()),
                unit_price: fmt.money(l.price.net_price, currency),
                vat_rate: rate,
                net: fmt.money(l.net_amount, currency),
            }
        })
        .collect()
}

/// The sums below the table, down to the amount to pay.
fn total_rows(
    invoice: &Invoice,
    branding: &Branding,
    fmt: &Formatter,
    currency: &str,
) -> Vec<Total> {
    let totals = &invoice.totals;
    let credit = invoice.kind == DocumentKind::CreditNote;
    let mut rows = vec![Total {
        label: "Summe der Positionen".into(),
        value: fmt.money(totals.line_total, currency),
        strong: false,
    }];
    if let Some(a) = totals.allowance_total.filter(|a| !a.is_zero()) {
        rows.push(Total {
            label: "Nachlässe".into(),
            value: format!("{}{}", fmt.minus(), fmt.money(a, currency)),
            strong: false,
        });
    }
    if let Some(c) = totals.charge_total.filter(|c| !c.is_zero()) {
        rows.push(Total {
            label: "Zuschläge".into(),
            value: format!("+{}", fmt.money(c, currency)),
            strong: false,
        });
    }
    let adjusted = rows.len() > 1;
    // without VAT the net is the sum of the positions, unless something was added or taken off
    if !branding.vat_free || adjusted {
        rows.push(Total {
            label: "Nettobetrag".into(),
            value: fmt.money(totals.taxable_total, currency),
            strong: false,
        });
    }
    for entry in invoice.vat_breakdown.iter().filter(|_| !branding.vat_free) {
        let rate = fmt.percent(&entry.rate.map_or_else(|| "0".to_owned(), |r| r.to_string()));
        rows.push(Total {
            label: format!(
                "Umsatzsteuer {rate} auf {}",
                fmt.money(entry.taxable_amount, currency)
            ),
            value: fmt.money(entry.tax_amount, currency),
            strong: false,
        });
    }
    if !branding.vat_free {
        rows.push(Total {
            label: "Gesamtbetrag".into(),
            value: fmt.money(totals.gross_total, currency),
            strong: true,
        });
    }
    if let Some(paid) = totals.paid.filter(|p| !p.is_zero()) {
        rows.push(Total {
            label: "Bereits bezahlt".into(),
            value: format!("{}{}", fmt.minus(), fmt.money(paid, currency)),
            strong: false,
        });
    }
    if let Some(rounding) = totals.rounding.filter(|r| !r.is_zero()) {
        rows.push(Total {
            label: "Rundung".into(),
            value: fmt.money(rounding, currency),
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
        value: fmt.money(totals.due, currency),
        strong: true,
    });
    rows
}

pub fn build(invoice: &Invoice, branding: &Branding, format: &Format) -> InvoiceView {
    let fmt = &Formatter::new(format);
    let currency = invoice.currency.as_ref().map_or("EUR", |c| c.as_str());
    let credit = invoice.kind == DocumentKind::CreditNote;
    let issue = invoice
        .issue_date
        .unwrap_or_else(|| Date::new(1970, 1, 1).unwrap());

    let infos = info_rows(invoice, branding, fmt, issue);
    let lines = line_views(invoice, branding, fmt, currency);
    let rows = total_rows(invoice, branding, fmt, currency);

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
        infos,
        intro: invoice
            .notes
            .iter()
            .filter_map(|n| n.note.clone())
            .collect(),
        lines,
        totals: rows,
        vat_notes,
        payment: payment_lines(invoice, currency, fmt, !branding.no_payment_term),
        legal: branding.legal.clone(),
        letter: Letter {
            intro: branding.intro.clone(),
            closing: branding.closing.clone(),
        },
        facts: Facts {
            number: invoice.number.clone().unwrap_or_default(),
            date: fmt.date(issue),
            service_date: invoice
                .delivery
                .as_ref()
                .and_then(|d| d.date)
                .map(|d| fmt.date(d)),
            same_day: invoice.delivery.as_ref().and_then(|d| d.date) == Some(issue),
            due_date: invoice
                .due_date
                .filter(|_| !branding.no_payment_term)
                .map(|d| fmt.date(d)),
            buyer_reference: invoice.buyer_reference.clone(),
            tax_number: invoice.seller.tax_registration.clone(),
            vat_id: invoice.seller.vat_identifier.clone(),
        },
        bank: bank_lines(invoice, fmt),
        page: PageView::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_layout(id: i64) -> CustomLayout {
        CustomLayout {
            id: Some(id),
            name: "Briefpapier".into(),
            fonts: Vec::new(),
            ..crate::layout::builtin()
        }
    }

    #[test]
    fn without_vat_the_document_shows_no_rate_no_vat_row_and_no_gross_sum() {
        let free = Settings {
            tax_rate: String::new(),
            ..crate::draft::fixtures::settings()
        };
        let draft = crate::draft::fixtures::draft();
        let invoice = crate::draft::build_invoice(&draft, &free, "RE-1").unwrap();
        let branding = Branding::for_draft(&draft, &free, &[]);

        let view = build(&invoice, &branding, &Format::default());

        assert!(view.lines.iter().all(|l| l.vat_rate.is_empty()));
        let labels: Vec<_> = view.totals.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["Summe der Positionen", "Zahlbetrag"]);
        assert_eq!(
            view.vat_notes,
            ["Gemäß § 19 UStG wird keine Umsatzsteuer berechnet."]
        );

        // with VAT everything is there as before
        let taxed = crate::draft::fixtures::settings();
        let invoice = crate::draft::build_invoice(&draft, &taxed, "RE-1").unwrap();
        let view = build(
            &invoice,
            &Branding::for_draft(&draft, &taxed, &[]),
            &Format::default(),
        );
        assert!(view.totals.iter().any(|t| t.label.contains("Umsatzsteuer")));
        assert!(view.lines[0].vat_rate.starts_with("19"));
    }

    #[test]
    fn without_payment_days_the_document_has_no_due_date_and_no_terms() {
        let none = Settings {
            payment_days: None,
            ..crate::draft::fixtures::settings()
        };
        let draft = crate::draft::fixtures::draft();
        let invoice = crate::draft::build_invoice(&draft, &none, "RE-1").unwrap();

        let view = build(
            &invoice,
            &Branding::for_draft(&draft, &none, &[]),
            &Format::default(),
        );

        assert_eq!(view.facts.due_date, None);
        assert!(view.facts.due_date.is_none());
        assert!(view.infos.iter().all(|i| i.key != "due-date"));
        assert!(view.payment.iter().all(|l| !l.contains("Zahlbar")));
        // how to pay is still there
        assert!(!view.payment.is_empty());

        let with = crate::draft::fixtures::settings();
        let invoice = crate::draft::build_invoice(&draft, &with, "RE-1").unwrap();
        let view = build(
            &invoice,
            &Branding::for_draft(&draft, &with, &[]),
            &Format::default(),
        );
        assert!(view.facts.due_date.is_some());
        assert!(view.payment.iter().any(|l| l.contains("Zahlbar")));
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
    fn a_reverse_charge_invoice_states_it() {
        let invoice = crate::invoice::golden::cases()
            .into_iter()
            .find(|(name, ..)| *name == "reverse-charge")
            .unwrap()
            .1;

        let view = build(&invoice, &Branding::default(), &Format::default());

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
    fn the_format_of_the_layout_reaches_every_figure_of_the_view() {
        let invoice =
            crate::invoice::golden::standard_invoice(crate::invoice::ProfileKind::En16931);
        let format = Format {
            currency_symbol: false,
            currency_after: false,
            currency_space: false,
            percent_space: false,
            decimal: ".".into(),
            thousands: ",".into(),
            date: "YYYY-MM-DD".into(),
            ..Format::default()
        };

        let view = build(&invoice, &Branding::default(), &format);

        let total = view.totals.last().unwrap();
        assert!(total.value.starts_with("EUR"), "{}", total.value);
        assert!(
            total.value.contains('.') && !total.value.contains(' '),
            "{}",
            total.value
        );
        assert!(view.totals.iter().any(|t| t.label.contains("19%")));
        assert!(view.lines[0].vat_rate.starts_with("19%"));
        assert!(view.facts.date.starts_with("2026-"), "{}", view.facts.date);
    }
}
