//! A context is one issuer: a company, later maybe a Verein or a private person. It has its own
//! seller, numbering and defaults (`Settings`) and a brand color the UI is tinted with.

use serde::{Deserialize, Serialize};

use crate::draft::decimal;
use crate::model::Settings;
use crate::numbering::Pattern;

/// Palettes of the UI library, `wa-brand-<name>` re-brands the whole UI. The first is the default.
pub const BRANDS: [&str; 9] = [
    "blue", "red", "orange", "yellow", "green", "cyan", "indigo", "purple", "pink",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Context {
    pub id: Option<i64>,
    /// Palette name of the UI library, empty on a new context: the store picks one
    #[serde(default)]
    pub brand: String,
    /// Set when the context was archived: hidden from the switch, its invoices stay
    #[serde(default)]
    pub archived_at: Option<String>,
    pub settings: Settings,
    /// What is missing before an invoice can be issued under it, as field paths of the form
    #[serde(default)]
    pub missing: Vec<String>,
    /// What only the `XRechnung` profile asks for on top
    #[serde(default)]
    pub missing_for_xrechnung: Vec<String>,
}

impl Context {
    pub fn new(settings: Settings, brand: &str) -> Self {
        Self {
            id: None,
            brand: brand.into(),
            archived_at: None,
            settings,
            missing: Vec::new(),
            missing_for_xrechnung: Vec::new(),
        }
    }

    /// Fills in what is missing, for the form and the guard.
    pub fn checked(mut self) -> Self {
        self.missing = self.settings.missing();
        self.missing_for_xrechnung = self.settings.missing_for_xrechnung();
        self
    }
}

/// What is missing in settings as they are typed, before they are saved
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Missing {
    pub missing: Vec<String>,
    pub missing_for_xrechnung: Vec<String>,
}

/// What happens with the customers, templates and layouts that belong to a context that is removed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Disposition {
    /// They stay and are available in every context
    Share,
    /// They go to another context
    Move { to: i64 },
    /// They are removed with it
    Delete,
}

/// What belongs to a context, so the user knows what removing or archiving touches
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ContextUsage {
    pub drafts: usize,
    pub exported: usize,
    pub customers: usize,
    pub templates: usize,
    pub layouts: usize,
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

impl Settings {
    /// Fields that must be filled in before an invoice can be issued, the same things the rule
    /// check refuses later (see `invoice::german`), found early for the form. Paths like the form's.
    pub fn missing(&self) -> Vec<String> {
        let seller = &self.seller;
        let address = &seller.address;
        let mut missing = Vec::new();
        let mut need = |field: &str, absent: bool| {
            if absent {
                missing.push(field.to_owned());
            }
        };
        need("seller.name", blank(&seller.name));
        need("seller.address.street", blank(&address.street));
        need("seller.address.post_code", blank(&address.post_code));
        need("seller.address.city", blank(&address.city));
        need("seller.address.country", address.country.trim().len() != 2);
        need(
            "seller.vat_id",
            seller.vat_id.as_deref().is_none_or(blank)
                && seller.tax_number.as_deref().is_none_or(blank),
        );
        need("seller.iban", blank(&seller.iban));
        need(
            "number_pattern",
            Pattern::parse(&self.number_pattern).is_err(),
        );
        need("number_reset", self.reset_problem().is_some());
        if self.vat_free() {
            need("vat_note", blank(&self.vat_note));
        } else {
            need(
                "tax_rate",
                !decimal(&self.tax_rate).is_some_and(|r| {
                    r > rust_decimal::Decimal::ZERO && r <= rust_decimal::Decimal::ONE_HUNDRED
                }),
            );
        }
        missing
    }

    /// What `XRechnung` wants of the seller on top: a contact (BR-DE-2) and its email
    pub fn missing_for_xrechnung(&self) -> Vec<String> {
        let seller = &self.seller;
        [
            ("seller.contact_name", blank(&seller.contact_name)),
            ("seller.phone", blank(&seller.phone)),
            ("seller.email", blank(&seller.email)),
        ]
        .into_iter()
        .filter(|(_, absent)| *absent)
        .map(|(field, _)| field.to_owned())
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::fixtures::settings;

    #[test]
    fn a_complete_seller_misses_nothing() {
        let settings = settings();
        assert_eq!(settings.missing(), Vec::<String>::new());
        assert_eq!(settings.missing_for_xrechnung(), Vec::<String>::new());
    }

    #[test]
    fn an_empty_context_misses_every_blocking_field() {
        let missing = Settings::default().missing();

        assert_eq!(
            missing,
            [
                "seller.name",
                "seller.address.street",
                "seller.address.post_code",
                "seller.address.city",
                // the country is DE by default
                "seller.vat_id",
                "seller.iban",
            ]
        );
        assert_eq!(
            Settings::default().missing_for_xrechnung(),
            ["seller.contact_name", "seller.phone", "seller.email"]
        );
    }

    #[test]
    fn each_blocking_field_is_reported_alone() {
        type Tweak = fn(&mut Settings);
        let cases: [(&str, Tweak); 10] = [
            ("seller.name", |s| s.seller.name = " ".into()),
            ("seller.address.street", |s| s.seller.address.street.clear()),
            ("seller.address.post_code", |s| {
                s.seller.address.post_code.clear();
            }),
            ("seller.address.city", |s| s.seller.address.city.clear()),
            ("seller.address.country", |s| {
                s.seller.address.country = "Deutschland".into();
            }),
            ("seller.iban", |s| s.seller.iban.clear()),
            ("number_pattern", |s| s.number_pattern = "RE".into()),
            ("number_reset", |s| {
                s.number_reset = crate::numbering::Reset::Month;
            }),
            ("tax_rate", |s| s.tax_rate = "abc".into()),
            ("vat_note", |s| {
                s.tax_rate.clear();
                s.vat_note.clear();
            }),
        ];
        for (field, tweak) in cases {
            let mut changed = settings();
            tweak(&mut changed);
            assert_eq!(changed.missing(), [field], "{field}");
        }
    }

    #[test]
    fn a_tax_number_is_enough_to_identify_the_seller() {
        let mut only_tax = settings();
        only_tax.seller.vat_id = None;
        only_tax.seller.tax_number = Some("123/456/78901".into());
        assert_eq!(only_tax.missing(), Vec::<String>::new());

        only_tax.seller.tax_number = Some("  ".into());
        assert_eq!(only_tax.missing(), ["seller.vat_id"]);
    }

    #[test]
    fn the_contact_is_only_missing_for_xrechnung() {
        let mut no_contact = settings();
        no_contact.seller.contact_name.clear();
        no_contact.seller.phone.clear();
        no_contact.seller.email.clear();

        assert_eq!(no_contact.missing(), Vec::<String>::new());
        assert_eq!(
            no_contact.missing_for_xrechnung(),
            ["seller.contact_name", "seller.phone", "seller.email"]
        );
    }
}
