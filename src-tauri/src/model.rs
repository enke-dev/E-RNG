//! What the app stores and what the UI edits. Money and quantities travel as strings so the
//! webview never rounds them, the Rust side parses them exactly (`draft.rs`).

use serde::{Deserialize, Serialize};

use crate::invisible::clean;
use crate::invoice::ProfileKind;

fn germany() -> String {
    "DE".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub street: String,
    pub post_code: String,
    pub city: String,
    #[serde(default = "germany")]
    pub country: String,
}

impl Address {
    /// Every field one line without what a paste brings along, see `Seller::cleaned`
    pub fn cleaned(self) -> Self {
        let line = |text: String| clean(&text, true);
        Self {
            street: line(self.street),
            post_code: line(self.post_code),
            city: line(self.city),
            country: line(self.country),
        }
    }
}

impl Default for Address {
    fn default() -> Self {
        Self {
            street: String::new(),
            post_code: String::new(),
            city: String::new(),
            country: germany(),
        }
    }
}

/// The issuer of the invoices, one per app.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Seller {
    pub name: String,
    pub address: Address,
    pub vat_id: Option<String>,
    /// The tax number (Steuernummer), for sellers without a VAT id
    pub tax_number: Option<String>,
    pub contact_name: String,
    pub phone: String,
    pub email: String,
    /// Mobile phone, fax and website: only for `{seller_*}` placeholders of layouts, not in the XML
    pub mobile: Option<String>,
    pub fax: Option<String>,
    pub website: Option<String>,
    pub iban: String,
    pub bic: Option<String>,
    pub account_holder: Option<String>,
    /// The name of the bank, only for `{bank_name}` of layouts, not in the XML
    pub bank_name: Option<String>,
    /// Register entry, managing directors etc., shown in the footer
    pub legal_info: Option<String>,
}

impl Seller {
    /// Without what a paste brings along and cannot be seen: no-break spaces, zero-width
    /// characters, line separators. Only the legal information has more than one line.
    pub fn cleaned(self) -> Self {
        let line = |text: String| clean(&text, true);
        let some_line = |text: Option<String>| text.map(line);
        Self {
            name: line(self.name),
            address: self.address.cleaned(),
            vat_id: some_line(self.vat_id),
            tax_number: some_line(self.tax_number),
            contact_name: line(self.contact_name),
            phone: line(self.phone),
            email: line(self.email),
            mobile: some_line(self.mobile),
            fax: some_line(self.fax),
            website: some_line(self.website),
            iban: line(self.iban),
            bic: some_line(self.bic),
            account_holder: some_line(self.account_holder),
            bank_name: some_line(self.bank_name),
            legal_info: self.legal_info.map(|text| clean(&text, false)),
        }
    }
}

/// The kinds of document that are written like an invoice and numbered on their own
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    #[default]
    Invoice,
    /// An offer (Angebot): a PDF only, no XML, no payment, valid until a date
    Offer,
}

impl Kind {
    /// As the `kind` columns spell it
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invoice => "invoice",
            Self::Offer => "offer",
        }
    }

    pub fn from_column(text: &str) -> Self {
        if text == "offer" {
            Self::Offer
        } else {
            Self::Invoice
        }
    }
}

/// What a kind of document calls itself: the headline, the headline of a draft and the label of
/// its number. Typed out because German compounds cannot be derived (`Offerte`, `Offertennummer`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wording {
    pub title: String,
    pub draft_title: String,
    pub number_label: String,
}

impl Wording {
    pub fn invoice() -> Self {
        Self {
            title: "Rechnung".into(),
            draft_title: "Rechnungsentwurf".into(),
            number_label: "Rechnungsnummer".into(),
        }
    }

    pub fn offer() -> Self {
        Self {
            title: "Angebot".into(),
            draft_title: "Angebotsentwurf".into(),
            number_label: "Angebotsnummer".into(),
        }
    }
}

impl Default for Wording {
    fn default() -> Self {
        Self::invoice()
    }
}

/// What offers have of their own; their layout, texts and numbers otherwise follow the invoices'
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OfferSettings {
    /// How offer numbers are made, see `numbering`
    pub number_pattern: String,
    /// When the counter of the offer numbers starts again; the pattern only formats it
    pub number_reset: crate::numbering::Reset,
    /// Days an offer is valid from its date; blank: no validity
    pub validity_days: Option<u32>,
    /// Texts of the letter, `{valid_until}` stands for the date the offer is valid until
    pub intro: Option<String>,
    pub closing: Option<String>,
    /// None: the layout of the invoices
    pub layout: Option<String>,
    pub wording: Wording,
}

impl Default for OfferSettings {
    fn default() -> Self {
        Self {
            number_pattern: crate::numbering::DEFAULT_OFFER_PATTERN.into(),
            number_reset: crate::numbering::Reset::default(),
            validity_days: Some(30),
            intro: None,
            closing: Some("Dieses Angebot gilt bis {valid_until}.".into()),
            layout: None,
            wording: Wording::offer(),
        }
    }
}

/// Older databases lack the newer fields, they fall back to the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub seller: Seller,
    /// VAT rate in percent, one for the whole app; blank: no VAT is charged (e.g. § 19 `UStG`), the
    /// invoices show none and say why with `vat_note`
    pub tax_rate: String,
    /// Why no VAT is charged, on invoices without it (the reason of the exemption, BT-120)
    pub vat_note: String,
    /// The label of the unit new positions start with. None, or a label the list of units no
    /// longer has: the first unit of the list.
    pub unit: Option<String>,
    /// Days until an invoice is due; blank: no due date and no payment terms are shown
    pub payment_days: Option<u32>,
    /// How invoice numbers are made, see `numbering`
    pub number_pattern: String,
    /// When the counter of the invoice numbers starts again; the pattern only formats it
    pub number_reset: crate::numbering::Reset,
    /// The start of the first business year, for `GGGG`, `GG`, `gggg`, `gg` and `Q` in the pattern:
    /// its day and month repeat every year, the calendar year applies before it. None: the
    /// business year is the calendar year.
    pub fiscal_year_start: Option<en16931::Date>,
    /// From before there was a pattern, only read: `normalized` turns it into one
    #[serde(skip_serializing)]
    pub number_prefix: Option<String>,
    /// From before the legal information was part of the seller, only read: `normalized` moves it,
    /// an exported invoice that froze it keeps showing it
    #[serde(skip_serializing)]
    pub footer: Option<String>,
    /// The layout of the invoice document (see `render::layouts`), invoices may choose another
    pub layout: String,
    /// Texts of the letter around the table, used unless an invoice has its own
    pub intro: Option<String>,
    pub closing: Option<String>,
    /// What invoices call themselves
    pub invoice_wording: Wording,
    /// New invoices name the offers they are linked to, each invoice may choose otherwise
    pub offer_reference: bool,
    /// Numbers, validity, texts and wording of offers
    pub offer: OfferSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            seller: Seller::default(),
            tax_rate: "19".into(),
            vat_note: "Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.".into(),
            unit: None,
            payment_days: Some(14),
            number_pattern: crate::numbering::DEFAULT_PATTERN.into(),
            number_reset: crate::numbering::Reset::default(),
            fiscal_year_start: None,
            number_prefix: None,
            footer: None,
            layout: "standard".into(),
            intro: None,
            closing: None,
            invoice_wording: Wording::invoice(),
            offer_reference: false,
            offer: OfferSettings::default(),
        }
    }
}

impl Settings {
    /// No VAT is charged: there is no rate
    pub fn vat_free(&self) -> bool {
        self.tax_rate.trim().is_empty()
    }

    /// The business year the numbers are made by
    pub fn calendar(&self) -> crate::numbering::Calendar {
        crate::numbering::Calendar {
            fiscal_year_start: self.fiscal_year_start,
        }
    }

    /// How the numbers of a kind are made
    pub fn pattern(&self, kind: Kind) -> &str {
        match kind {
            Kind::Invoice => &self.number_pattern,
            Kind::Offer => &self.offer.number_pattern,
        }
    }

    /// When the counter of a kind's numbers starts again
    pub fn reset(&self, kind: Kind) -> crate::numbering::Reset {
        match kind {
            Kind::Invoice => self.number_reset,
            Kind::Offer => self.offer.number_reset,
        }
    }

    /// What keeps the reset of a kind's counter from working with its pattern (see
    /// `numbering::reset_problem`), none when it does or when the pattern is broken anyway
    pub fn reset_problem(&self, kind: Kind) -> Option<&'static str> {
        let pattern = crate::numbering::Pattern::parse(self.pattern(kind)).ok()?;
        crate::numbering::reset_problem(
            &pattern,
            self.reset(kind),
            self.calendar(),
            crate::utils::issue_day(&crate::utils::today()).year(),
        )
    }

    /// The default layout of a kind, offers without one of their own take the invoices'
    pub fn layout_for(&self, kind: Kind) -> &str {
        match kind {
            Kind::Invoice => &self.layout,
            Kind::Offer => self
                .offer
                .layout
                .as_deref()
                .filter(|layout| !layout.is_empty())
                .unwrap_or(&self.layout),
        }
    }

    pub fn intro(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Invoice => self.intro.as_deref(),
            Kind::Offer => self.offer.intro.as_deref(),
        }
    }

    pub fn closing(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Invoice => self.closing.as_deref(),
            Kind::Offer => self.offer.closing.as_deref(),
        }
    }

    /// The wording of a kind, a part left blank is the default one
    pub fn wording(&self, kind: Kind) -> Wording {
        let (own, default) = match kind {
            Kind::Invoice => (&self.invoice_wording, Wording::invoice()),
            Kind::Offer => (&self.offer.wording, Wording::offer()),
        };
        let pick = |own: &str, default: String| {
            Some(own.trim())
                .filter(|o| !o.is_empty())
                .map_or(default, str::to_owned)
        };
        Wording {
            title: pick(&own.title, default.title.clone()),
            draft_title: pick(&own.draft_title, default.draft_title.clone()),
            number_label: pick(&own.number_label, default.number_label.clone()),
        }
    }

    /// A blank draft dated `today` with what the settings give a document of the kind: its layout,
    /// its texts and, for an offer, the days it is valid.
    pub fn blank_draft(&self, kind: Kind, today: &str) -> InvoiceDraft {
        let mut draft = InvoiceDraft::blank(today);
        draft.layout = Some(self.layout_for(kind).to_owned());
        draft.intro = Some(self.intro(kind).unwrap_or_default().to_owned());
        draft.closing = Some(self.closing(kind).unwrap_or_default().to_owned());
        match kind {
            Kind::Offer => draft.validity_days = self.offer.validity_days,
            Kind::Invoice => draft.offer_reference = self.offer_reference,
        }
        draft
    }

    /// Settings of an older version have a prefix and no pattern: the numbers stay what they were.
    pub fn normalized(mut self) -> Self {
        if let Some(prefix) = self.number_prefix.take() {
            // brackets cannot be written inside brackets
            let prefix = prefix.replace(['[', ']'], "");
            self.number_pattern = if prefix.is_empty() {
                "YYYY-NNN".into()
            } else {
                format!("[{prefix}]YYYY-NNN")
            };
        }
        if let Some(footer) = self.footer.take() {
            let empty = |text: Option<&str>| text.is_none_or(|t| t.trim().is_empty());
            if empty(self.seller.legal_info.as_deref()) {
                self.seller.legal_info = Some(footer).filter(|f| !f.trim().is_empty());
            }
        }
        self
    }
}

pub use crate::layout::{CustomLayout, FontFile, Format, Insets, LayoutUsage};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Customer {
    pub id: Option<i64>,
    /// The context it belongs to, none: it is shared by all. Stored beside the entry, not in it.
    #[serde(default)]
    pub context_id: Option<i64>,
    pub name: String,
    pub address: Address,
    pub vat_id: Option<String>,
    /// The address electronic invoices are announced to (BT-49)
    pub email: Option<String>,
    /// Leitweg-ID for public customers, copied into new invoices as buyer reference
    pub buyer_reference: Option<String>,
}

impl Customer {
    /// Every field one line without what a paste brings along, see `Seller::cleaned`
    pub fn cleaned(self) -> Self {
        let some_line = |text: Option<String>| text.map(|text| clean(&text, true));
        Self {
            name: clean(&self.name, true),
            address: self.address.cleaned(),
            vat_id: some_line(self.vat_id),
            email: some_line(self.email),
            buyer_reference: some_line(self.buyer_reference),
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Position {
    pub description: String,
    /// Empty means 1
    pub quantity: Option<String>,
    /// UN/ECE rec. 20 code, empty means C62 (older data)
    pub unit: Option<String>,
    /// The label as it was picked, so renaming or deleting a unit never changes a created invoice;
    /// empty (older data): what the code was printed as then
    #[serde(default)]
    pub unit_label: Option<String>,
    /// VAT rate in percent as the position picked it, kept in it like the unit so a created
    /// invoice never changes with the list of rates; none (or blank): the context's default rate
    #[serde(default)]
    pub tax_rate: Option<String>,
    /// The label the rate was picked with, only for showing it again
    #[serde(default)]
    pub tax_label: Option<String>,
    /// Net price per unit
    pub price: String,
}

impl Position {
    /// The unit code, blank is piece
    pub fn unit_code(&self) -> &str {
        self.unit
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .unwrap_or("C62")
    }

    /// The label the unit is shown with: the one it was picked with, else what the code was
    /// printed as before units had labels
    pub fn unit_label(&self) -> String {
        self.unit_label
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map_or_else(
                || crate::units::legacy_label(self.unit_code()),
                str::to_owned,
            )
    }

    /// The same position with code and label filled in, for what is read from the store: older
    /// rows have neither, and are never rewritten for it.
    pub fn normalized(self) -> Self {
        Self {
            unit: Some(self.unit_code().to_owned()),
            unit_label: Some(self.unit_label()),
            ..self
        }
    }

    /// The same position as another one: description (case aside), quantity (empty is 1), unit
    /// (code and label, empty is piece), rate (none is the default, `7` is `7,0`, with its label) and price compare
    /// as the user means them, `2` is `2,00`.
    pub fn same_as(&self, other: &Position) -> bool {
        use crate::draft::decimal;
        use crate::rates;
        let quantity = |p: &Position| {
            p.quantity
                .as_deref()
                .map(str::trim)
                .filter(|q| !q.is_empty())
                .map_or(Some(rust_decimal::Decimal::ONE), decimal)
        };
        let price = |p: &Position| decimal(&p.price);
        let rate = |p: &Position| {
            p.tax_rate
                .as_deref()
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(|r| {
                    (
                        rates::canonical(r).unwrap_or_else(|| r.to_owned()),
                        p.tax_label
                            .as_deref()
                            .map(str::trim)
                            .unwrap_or_default()
                            .to_owned(),
                    )
                })
        };
        self.description.trim().to_lowercase() == other.description.trim().to_lowercase()
            && quantity(self) == quantity(other)
            && self.unit_code() == other.unit_code()
            && self.unit_label() == other.unit_label()
            && rate(self) == rate(other)
            && price(self) == price(other)
    }
}

/// A saved position the invoice form can start from, edited like a customer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PositionTemplate {
    pub id: Option<i64>,
    /// The context it belongs to, none: it is shared by all
    #[serde(default)]
    pub context_id: Option<i64>,
    #[serde(flatten)]
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceDraft {
    /// A copy: later edits of the customer do not touch existing invoices
    pub customer: Customer,
    /// ISO date, `2026-10-17`
    pub issue_date: String,
    /// Date of service, ISO date; the start when there is an end
    pub delivery_date: String,
    /// End of the service period, ISO date; none: the service was on one day. Blank while it is
    /// being typed, the period is still chosen.
    #[serde(default)]
    pub delivery_end: Option<String>,
    /// Offers: how many days from its date it is valid; none: no validity
    #[serde(default)]
    pub validity_days: Option<u32>,
    pub positions: Vec<Position>,
    pub profile: ProfileKind,
    pub buyer_reference: Option<String>,
    pub note: Option<String>,
    /// A layout for this invoice only, empty means the one from the settings
    #[serde(default)]
    pub layout: Option<String>,
    /// Texts of this invoice, new invoices start with those of the settings; `None` (made before the
    /// texts existed) takes the settings, an empty text is no text
    #[serde(default)]
    pub intro: Option<String>,
    #[serde(default)]
    pub closing: Option<String>,
    /// The settings an exported invoice was made with, so it can be produced again unchanged
    #[serde(default)]
    pub frozen: Option<Settings>,
    /// The own layout it was made with, frozen like the settings
    #[serde(default)]
    pub frozen_layout: Option<CustomLayout>,
    /// Invoices: the document names the numbers of the offers it is linked to
    #[serde(default)]
    pub offer_reference: bool,
    /// The numbers of the linked offers, as they were when the invoice was created
    #[serde(default)]
    pub frozen_offers: Vec<String>,
}

impl InvoiceDraft {
    /// The day an offer is valid until, ISO date: its date plus the validity days. None without
    /// days or with no date to count from.
    pub fn valid_until(&self) -> Option<String> {
        let days = self.validity_days?;
        let issue = chrono::NaiveDate::parse_from_str(&self.issue_date, "%Y-%m-%d").ok()?;
        let until = issue.checked_add_days(chrono::Days::new(u64::from(days)))?;
        Some(until.format("%Y-%m-%d").to_string())
    }

    pub fn blank(today: &str) -> Self {
        Self {
            customer: Customer::default(),
            issue_date: today.into(),
            delivery_date: today.into(),
            delivery_end: None,
            validity_days: None,
            positions: vec![Position::default()],
            profile: ProfileKind::default(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Draft,
    Exported,
}

/// One of the files made when an invoice was created, kept in the app's data folder
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredFile {
    /// Relative to the data folder
    pub path: String,
    /// What the file must still be, checked before it is handed out
    pub sha256: String,
}

/// The originals of an invoice: what was validated is what is archived and what gets sent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceFiles {
    pub pdf: StoredFile,
    /// None for an offer, it has a PDF only
    #[serde(default)]
    pub xml: Option<StoredFile>,
    /// Made again from the frozen data, for an invoice that was exported before files were kept
    #[serde(default)]
    pub regenerated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoredInvoice {
    pub id: i64,
    /// The context (issuer) it was made under, for good
    pub context_id: i64,
    /// What it is, for good: the table holds invoices and offers
    pub kind: Kind,
    /// Assigned when the invoice is exported, drafts have none
    pub number: Option<String>,
    pub status: Status,
    pub draft: InvoiceDraft,
    pub total: Option<String>,
    pub exported_at: Option<String>,
    /// ISO date the invoice was paid on, set by hand after the export
    pub paid_at: Option<String>,
    pub updated_at: String,
    /// Taken out of the list, it stays as it is and can be put back
    pub archived_at: Option<String>,
    /// The originals, none for a draft and for an invoice exported before they were kept
    pub files: Option<InvoiceFiles>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListEntry {
    pub id: i64,
    pub context_id: i64,
    pub kind: Kind,
    /// The seller of its context, tells invoices of different contexts apart in a mixed list
    pub seller_name: String,
    pub number: Option<String>,
    pub status: Status,
    pub customer_name: String,
    pub issue_date: String,
    /// Offers: the day it is valid until
    pub valid_until: Option<String>,
    pub total: Option<String>,
    pub paid_at: Option<String>,
    pub updated_at: String,
    pub archived_at: Option<String>,
    /// How many documents of the other kind it is linked to
    pub linked: u32,
}

/// The other end of a link between an offer and an invoice
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Linked {
    pub id: i64,
    pub kind: Kind,
    pub number: Option<String>,
    pub status: Status,
    pub customer_name: String,
    pub issue_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_stored_by_an_older_version_get_the_new_defaults() {
        let old = r#"{"seller":{"name":"A","address":{"street":"","post_code":"","city":"","country":"DE"},
            "vat_id":null,"tax_number":null,"contact_name":"","phone":"","email":"","iban":"","bic":null,
            "account_holder":null,"legal_info":null},"tax_rate":"19","payment_days":14,
            "number_prefix":"RE-","footer":null}"#;

        let settings: Settings = serde_json::from_str(old).unwrap();

        assert_eq!(settings.seller.name, "A");
        assert_eq!(settings.seller.mobile, None);
        assert_eq!(settings.seller.fax, None);
        assert_eq!(settings.seller.website, None);
        assert_eq!(settings.seller.bank_name, None);
        assert_eq!(settings.layout, "standard");
        assert_eq!(settings.intro, None);
        assert_eq!(settings.number_reset, crate::numbering::Reset::FiscalYear);
    }

    #[test]
    fn settings_from_before_the_default_unit_have_none() {
        let settings: Settings = serde_json::from_str(r#"{"tax_rate":"19"}"#).unwrap();

        assert_eq!(settings.unit, None);
    }

    #[test]
    fn the_reset_of_the_counter_is_stored_by_name() {
        use crate::numbering::Reset;

        let json = |reset| {
            serde_json::to_value(Settings {
                number_reset: reset,
                ..Settings::default()
            })
            .unwrap()["number_reset"]
                .clone()
        };

        assert_eq!(json(Reset::Never), "never");
        assert_eq!(json(Reset::FiscalYear), "fiscal_year");
        assert_eq!(json(Reset::Quarter), "quarter");
        assert_eq!(json(Reset::Month), "month");
        assert_eq!(
            serde_json::to_value(Settings::default()).unwrap()["number_reset"],
            "fiscal_year"
        );
    }

    #[test]
    fn settings_of_before_offers_have_the_offer_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"tax_rate":"19"}"#).unwrap();

        assert_eq!(settings.offer.number_pattern, "[AN-]YYYY-NNN");
        assert_eq!(settings.offer.validity_days, Some(30));
        assert!(settings
            .offer
            .closing
            .as_deref()
            .is_some_and(|text| text.contains("{valid_until}")));
        assert_eq!(settings.offer.layout, None);
        assert_eq!(settings.wording(Kind::Invoice), Wording::invoice());
        assert_eq!(settings.wording(Kind::Offer), Wording::offer());
        assert_eq!(settings.invoice_wording.number_label, "Rechnungsnummer");
        assert_eq!(settings.offer.wording.number_label, "Angebotsnummer");

        // a validity left blank stays blank
        let blank: Settings = serde_json::from_str(
            r#"{"offer":{"validity_days":null,"wording":{"title":"Offerte"}}}"#,
        )
        .unwrap();
        assert_eq!(blank.offer.validity_days, None);
        assert_eq!(blank.offer.number_pattern, "[AN-]YYYY-NNN");
        assert_eq!(blank.wording(Kind::Offer).title, "Offerte");
    }

    #[test]
    fn a_wording_left_blank_is_the_default_one() {
        let mut settings = Settings::default();
        settings.offer.wording.title = "  ".into();
        settings.offer.wording.draft_title = "Offertenentwurf".into();
        settings.offer.wording.number_label.clear();

        let wording = settings.wording(Kind::Offer);

        assert_eq!(wording.title, "Angebot");
        assert_eq!(wording.draft_title, "Offertenentwurf");
        assert_eq!(wording.number_label, "Angebotsnummer");
    }

    #[test]
    fn the_kinds_are_written_kebab_case_and_read_from_their_column() {
        assert_eq!(serde_json::to_string(&Kind::Offer).unwrap(), "\"offer\"");
        assert_eq!(Kind::from_column("offer"), Kind::Offer);
        assert_eq!(Kind::from_column(Kind::Invoice.as_str()), Kind::Invoice);
    }

    #[test]
    fn a_blank_draft_has_what_the_settings_give_its_kind() {
        let mut settings = Settings {
            layout: "custom-2".into(),
            intro: Some("Rechnungstext".into()),
            closing: None,
            ..Settings::default()
        };

        let invoice = settings.blank_draft(Kind::Invoice, "2026-10-17");

        assert_eq!(invoice.issue_date, "2026-10-17");
        assert_eq!(invoice.layout.as_deref(), Some("custom-2"));
        assert_eq!(invoice.intro.as_deref(), Some("Rechnungstext"));
        assert_eq!(invoice.closing.as_deref(), Some(""));
        assert_eq!(invoice.validity_days, None);
        assert!(!invoice.offer_reference);
        settings.offer_reference = true;
        assert!(
            settings
                .blank_draft(Kind::Invoice, "2026-10-17")
                .offer_reference
        );
        // an offer names no offers
        assert!(
            !settings
                .blank_draft(Kind::Offer, "2026-10-17")
                .offer_reference
        );
        settings.offer_reference = false;

        // an offer without a layout of its own takes the invoices'
        let offer = settings.blank_draft(Kind::Offer, "2026-10-17");
        assert_eq!(offer.layout.as_deref(), Some("custom-2"));
        assert_eq!(offer.intro.as_deref(), Some(""));
        assert_eq!(
            offer.closing.as_deref(),
            Some("Dieses Angebot gilt bis {valid_until}.")
        );
        assert_eq!(offer.validity_days, Some(30));

        settings.offer.layout = Some("standard".into());
        settings.offer.validity_days = None;
        let own = settings.blank_draft(Kind::Offer, "2026-10-17");
        assert_eq!(own.layout.as_deref(), Some("standard"));
        assert_eq!(own.validity_days, None);
        // over a month and a year end
    }

    #[test]
    fn an_offer_is_valid_until_its_date_plus_the_days() {
        let mut draft = InvoiceDraft::blank("2026-10-17");
        assert_eq!(draft.valid_until(), None);

        draft.validity_days = Some(30);
        assert_eq!(draft.valid_until().as_deref(), Some("2026-11-16"));

        // over a month and a year end, and the same day for none
        draft.issue_date = "2026-12-15".into();
        assert_eq!(draft.valid_until().as_deref(), Some("2027-01-14"));
        draft.validity_days = Some(0);
        assert_eq!(draft.valid_until().as_deref(), Some("2026-12-15"));

        draft.issue_date = "kein Datum".into();
        assert_eq!(draft.valid_until(), None);
    }

    #[test]
    fn a_prefix_from_an_older_version_becomes_a_pattern() {
        let stored = |prefix: &str| Settings {
            number_prefix: Some(prefix.into()),
            ..Settings::default()
        };

        assert_eq!(stored("RE-").normalized().number_pattern, "[RE-]YYYY-NNN");
        assert_eq!(stored("R[1]").normalized().number_pattern, "[R1]YYYY-NNN");
        assert_eq!(stored("").normalized().number_pattern, "YYYY-NNN");
        // what has a pattern is left alone, and the prefix is not written again
        let new = Settings::default().normalized();
        assert_eq!(new.number_pattern, crate::numbering::DEFAULT_PATTERN);
        assert!(!serde_json::to_string(&stored("RE-"))
            .unwrap()
            .contains("number_prefix"));
    }

    #[test]
    fn a_cleaned_seller_has_one_line_per_field_but_the_legal_information() {
        let seller = Seller {
            name: "Muster\u{00A0}GmbH\u{200B}".into(),
            address: Address {
                street: "Musterstr.\u{2028}1".into(),
                ..Address::default()
            },
            phone: "+49\u{202F}221\t1".into(),
            iban: "\u{FEFF}DE89 3704".into(),
            mobile: Some("0171\u{2011}5".into()),
            legal_info: Some("HRB 1\u{2029}GF: A\nUSt".into()),
            ..Seller::default()
        }
        .cleaned();

        assert_eq!(seller.name, "Muster GmbH");
        assert_eq!(seller.address.street, "Musterstr. 1");
        assert_eq!(seller.address.country, "DE");
        assert_eq!(seller.phone, "+49 221 1");
        assert_eq!(seller.iban, "DE89 3704");
        assert_eq!(seller.mobile.as_deref(), Some("0171-5"));
        assert_eq!(seller.fax, None);
        assert_eq!(seller.legal_info.as_deref(), Some("HRB 1\nGF: A\nUSt"));
    }

    #[test]
    fn a_cleaned_customer_has_one_line_per_field() {
        let customer = Customer {
            id: Some(4),
            context_id: Some(2),
            name: "Beispiel\u{00A0}Kunde\u{200B} GmbH".into(),
            address: Address {
                street: "Musterweg\u{2028}1".into(),
                post_code: "\u{FEFF}12345".into(),
                city: "Musterstadt".into(),
                country: "DE".into(),
            },
            vat_id: None,
            email: Some("rechnung@kunde.example\u{200B}".into()),
            buyer_reference: Some("04011000\u{2011}1234512345\u{2011}06".into()),
        }
        .cleaned();

        assert_eq!(customer.name, "Beispiel Kunde GmbH");
        assert_eq!(customer.address.street, "Musterweg 1");
        assert_eq!(customer.address.post_code, "12345");
        assert_eq!(customer.email.as_deref(), Some("rechnung@kunde.example"));
        assert_eq!(
            customer.buyer_reference.as_deref(),
            Some("04011000-1234512345-06")
        );
        assert_eq!((customer.id, customer.context_id), (Some(4), Some(2)));
    }

    #[test]
    fn a_footer_from_an_older_version_becomes_the_legal_information() {
        let with = |footer: &str, legal: Option<&str>| {
            let mut settings = Settings {
                footer: Some(footer.into()),
                ..Settings::default()
            };
            settings.seller.legal_info = legal.map(Into::into);
            settings.normalized()
        };
        let legal = |settings: &Settings| settings.seller.legal_info.clone();

        assert_eq!(
            legal(&with("AG Köln HRB 1", None)).as_deref(),
            Some("AG Köln HRB 1")
        );
        // what is there stays, an empty footer is nothing
        assert_eq!(legal(&with("alt", Some("neu"))).as_deref(), Some("neu"));
        assert_eq!(legal(&with("  ", None)), None);
        assert!(!serde_json::to_string(&with("x", None))
            .unwrap()
            .contains("\"footer\""));
    }

    #[test]
    fn drafts_stored_by_an_older_version_have_no_layout_or_texts() {
        let mut json = serde_json::to_value(InvoiceDraft::blank("2026-10-17")).unwrap();
        for key in ["layout", "intro", "closing", "validity_days"] {
            json.as_object_mut().unwrap().remove(key);
        }

        let draft: InvoiceDraft = serde_json::from_value(json).unwrap();

        assert_eq!(
            (
                draft.layout,
                draft.intro,
                draft.closing,
                draft.validity_days
            ),
            (None, None, None, None)
        );
    }
}
