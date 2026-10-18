//! What the app stores and what the UI edits. Money and quantities travel as strings so the
//! webview never rounds them, the Rust side parses them exactly (`draft.rs`).

use std::path::PathBuf;

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
    /// VAT rate in percent, one for the whole app
    pub tax_rate: String,
    pub payment_days: u32,
    /// Invoice numbers are `<prefix><year>-<sequence>`
    pub number_prefix: String,
    pub footer: Option<String>,
    pub logo: Option<PathBuf>,
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
            payment_days: 14,
            number_prefix: "RE-".into(),
            footer: None,
            logo: None,
            layout: "standard".into(),
            intro: None,
            closing: None,
        }
    }
}

/// Distances from the page edge to the content, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for Insets {
    fn default() -> Self {
        Self {
            top: 20.0,
            right: 20.0,
            bottom: 25.0,
            left: 25.0,
        }
    }
}

/// The stationery of a layout: the pages of an uploaded PDF, converted to SVG (Typst cannot put
/// a PDF behind PDF/A pages). One page covers all pages of the invoice, a second one is for
/// every page after the first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Background {
    /// Name of the uploaded file, shown in the form
    pub name: String,
    pub first: PathBuf,
    pub rest: Option<PathBuf>,
}

/// An uploaded font file and the family Typst knows it by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontFile {
    pub family: String,
    pub path: PathBuf,
}

/// An invoice that is made with an own layout, what keeps the layout from being deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutUsage {
    pub invoice_id: i64,
    /// `None` for a draft
    pub number: Option<String>,
    pub customer: String,
}

/// A layout the user made: the built-in design on the user's stationery and font.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomLayout {
    pub id: Option<i64>,
    pub name: String,
    pub background: Option<Background>,
    /// Where the content may go on the pages after the first, and on the first unless it has its own
    pub margins: Insets,
    /// The first page may have insets of its own on all four sides, e.g. for a letterhead
    pub first_margins: Option<Insets>,
    /// The files of one font family, all of them are made available to Typst
    pub fonts: Vec<FontFile>,
    /// Out of the selection, kept for the invoices that were made with it
    #[serde(default)]
    pub archived: bool,
}

impl CustomLayout {
    /// The id the settings and invoices refer to it by
    pub fn key(&self) -> String {
        format!("custom-{}", self.id.unwrap_or_default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Customer {
    pub id: Option<i64>,
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
            "number_prefix":"RE-","footer":null,"logo":null}"#;

        let settings: Settings = serde_json::from_str(old).unwrap();

        assert_eq!(settings.seller.name, "A");
        assert_eq!(settings.layout, "standard");
        assert_eq!(settings.intro, None);
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
