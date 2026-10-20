//! What the app stores and what the UI edits. Money and quantities travel as strings so the
//! webview never rounds them, the Rust side parses them exactly (`draft.rs`).

use serde::{Deserialize, Serialize};

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
    pub iban: String,
    pub bic: Option<String>,
    pub account_holder: Option<String>,
    /// Register entry, managing directors etc., shown in the footer
    pub legal_info: Option<String>,
}

/// Older databases lack the newer fields, they fall back to the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub seller: Seller,
    /// VAT rate in percent, one for the whole app; blank: no VAT is charged (e.g. § 19 UStG), the
    /// invoices show none and say why with `vat_note`
    pub tax_rate: String,
    /// Why no VAT is charged, on invoices without it (the reason of the exemption, BT-120)
    pub vat_note: String,
    /// Days until an invoice is due; blank: no due date and no payment terms are shown
    pub payment_days: Option<u32>,
    /// How invoice numbers are made, see `numbering`
    pub number_pattern: String,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            seller: Seller::default(),
            tax_rate: "19".into(),
            vat_note: "Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.".into(),
            payment_days: Some(14),
            number_pattern: crate::numbering::DEFAULT_PATTERN.into(),
            number_prefix: None,
            footer: None,
            layout: "standard".into(),
            intro: None,
            closing: None,
        }
    }
}

impl Settings {
    /// No VAT is charged: there is no rate
    pub fn vat_free(&self) -> bool {
        self.tax_rate.trim().is_empty()
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
            let empty = |text: &Option<String>| text.as_deref().is_none_or(|t| t.trim().is_empty());
            if empty(&self.seller.legal_info) {
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Position {
    pub description: String,
    /// Empty means 1
    pub quantity: Option<String>,
    /// UN/ECE unit code, empty means piece
    pub unit: Option<String>,
    /// Net price per unit
    pub price: String,
}

impl Position {
    /// The same position as another one: description (case aside), quantity (empty is 1), unit
    /// (empty is piece) and price compare as the user means them, `2` is `2,00`.
    pub fn same_as(&self, other: &Position) -> bool {
        use crate::draft::decimal;
        let quantity = |p: &Position| {
            p.quantity
                .as_deref()
                .map(str::trim)
                .filter(|q| !q.is_empty())
                .map_or(Some(rust_decimal::Decimal::ONE), decimal)
        };
        let unit = |p: &Position| {
            p.unit
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .unwrap_or("C62")
                .to_owned()
        };
        let price = |p: &Position| decimal(&p.price);
        self.description.trim().to_lowercase() == other.description.trim().to_lowercase()
            && quantity(self) == quantity(other)
            && unit(self) == unit(other)
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
    /// Date of service, ISO date
    pub delivery_date: String,
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
}

impl InvoiceDraft {
    pub fn blank(today: &str) -> Self {
        Self {
            customer: Customer::default(),
            issue_date: today.into(),
            delivery_date: today.into(),
            positions: vec![Position::default()],
            profile: ProfileKind::default(),
            buyer_reference: None,
            note: None,
            layout: None,
            intro: None,
            closing: None,
            frozen: None,
            frozen_layout: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Draft,
    Exported,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoredInvoice {
    pub id: i64,
    /// The context (issuer) it was made under, for good
    pub context_id: i64,
    /// Assigned when the invoice is exported, drafts have none
    pub number: Option<String>,
    pub status: Status,
    pub draft: InvoiceDraft,
    pub total: Option<String>,
    pub exported_at: Option<String>,
    /// ISO date the invoice was paid on, set by hand after the export
    pub paid_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListEntry {
    pub id: i64,
    pub context_id: i64,
    /// The seller of its context, tells invoices of different contexts apart in a mixed list
    pub seller_name: String,
    pub number: Option<String>,
    pub status: Status,
    pub customer_name: String,
    pub issue_date: String,
    pub total: Option<String>,
    pub paid_at: Option<String>,
    pub updated_at: String,
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
        assert_eq!(settings.layout, "standard");
        assert_eq!(settings.intro, None);
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
        for key in ["layout", "intro", "closing"] {
            json.as_object_mut().unwrap().remove(key);
        }

        let draft: InvoiceDraft = serde_json::from_value(json).unwrap();

        assert_eq!(
            (draft.layout, draft.intro, draft.closing),
            (None, None, None)
        );
    }
}
