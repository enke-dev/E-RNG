//! VAT rates: the list each context keeps for its positions to pick from (a label and a rate in
//! percent), and how a rate is read.
//!
//! A position that picked a rate carries its value and its label (`Position::tax_rate`,
//! `Position::tax_label`), so changing or deleting a rate never changes a created invoice. A
//! position without one takes the context's default rate (`Settings::tax_rate`, the value only),
//! which is one of the list as well. There are no VAT categories or exemption reasons here: a rate
//! is only a percentage with a name.

use anyhow::{bail, Result};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::draft::decimal;
use crate::units::{DraftRef, TemplateRef};

/// A rate of a context's list
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Rate {
    /// None while it is new
    pub id: Option<i64>,
    /// The context whose list it is in, fixed once stored
    pub context_id: i64,
    /// What the rate is called, unique in the list (case aside)
    pub label: String,
    /// Percent as stored: dot as the separator, no trailing zeros (`19`, `5.5`)
    pub rate: String,
}

/// A rate without a place: what a new context starts with
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct RateDef {
    pub label: String,
    pub rate: String,
}

/// What every context starts with: label and rate
pub const DEFAULTS: [(&str, &str); 2] = [("Regelsatz", "19"), ("Ermäßigt", "7")];

/// What a rate is called that a context charges but its list lacks (it comes from before there
/// was a list)
pub const CHARGED_LABEL: &str = "Steuersatz";

/// The defaults as a list to store
pub fn defaults() -> Vec<RateDef> {
    DEFAULTS
        .iter()
        .map(|(label, rate)| RateDef {
            label: (*label).into(),
            rate: (*rate).into(),
        })
        .collect()
}

/// A rate as typed, comma or dot: more than 0 and at most 100
pub fn parse(text: &str) -> Option<Decimal> {
    decimal(text).filter(|rate| *rate > Decimal::ZERO && *rate <= Decimal::ONE_HUNDRED)
}

/// The rate the way it is stored, `19,0` and `19.00` are both `19`
pub fn canonical(text: &str) -> Option<String> {
    parse(text).map(|rate| rate.normalize().to_string())
}

/// A rate as it may be stored: trimmed, with a rate in its canonical form and a label that none of
/// `others` (the rest of the list) has, case aside; the rate is unique as well, so the context's
/// default (a rate without a label) names one entry.
pub fn check(rate: &RateDef, others: &[Rate]) -> Result<RateDef> {
    let label = rate.label.trim();
    if label.is_empty() {
        bail!("Eine Bezeichnung ist erforderlich.");
    }
    let Some(value) = canonical(&rate.rate) else {
        bail!("Der Steuersatz muss größer als 0 und höchstens 100 sein (in Prozent).");
    };
    if others
        .iter()
        .any(|o| o.label.trim().to_lowercase() == label.to_lowercase())
    {
        bail!("Die Bezeichnung „{label}“ gibt es schon.");
    }
    if others
        .iter()
        .any(|o| canonical(&o.rate).as_deref() == Some(value.as_str()))
    {
        bail!("Den Steuersatz {value} % gibt es schon.");
    }
    Ok(RateDef {
        label: label.into(),
        rate: value,
    })
}

/// Where a rate is used by what a change reaches: drafts and saved positions, and the default of
/// the context. Created, imported and archived documents are never touched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RateUsage {
    pub templates: Vec<TemplateRef>,
    pub drafts: Vec<DraftRef>,
    /// It is the rate the context's positions take when they pick none
    pub default: bool,
}

impl RateUsage {
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty() && self.drafts.is_empty() && !self.default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(label: &str, rate: &str) -> Rate {
        Rate {
            id: Some(1),
            context_id: 1,
            label: label.into(),
            rate: rate.into(),
        }
    }

    fn def(label: &str, rate: &str) -> RateDef {
        RateDef {
            label: label.into(),
            rate: rate.into(),
        }
    }

    #[test]
    fn a_rate_is_more_than_0_and_at_most_100_with_comma_or_dot() {
        for (text, ok) in [
            ("19", true),
            ("5,5", true),
            ("5.5", true),
            (" 7 ", true),
            ("100", true),
            ("0,01", true),
            ("0", false),
            ("-7", false),
            ("100,1", false),
            ("", false),
            ("abc", false),
        ] {
            assert_eq!(parse(text).is_some(), ok, "{text:?}");
        }
    }

    #[test]
    fn a_rate_is_stored_without_trailing_zeros_and_with_a_dot() {
        for (text, stored) in [
            ("19", "19"),
            ("19,00", "19"),
            ("19.0", "19"),
            ("5,50", "5.5"),
            ("0,7", "0.7"),
            ("100", "100"),
        ] {
            assert_eq!(canonical(text).as_deref(), Some(stored), "{text:?}");
        }
    }

    #[test]
    fn the_label_is_trimmed_required_and_unique_ignoring_case() {
        let others = [listed("Regelsatz", "19")];

        assert_eq!(
            check(&def("  Ermäßigt ", " 7,0 "), &others).unwrap(),
            def("Ermäßigt", "7")
        );
        assert!(check(&def("  ", "7"), &others).is_err());
        assert!(check(&def("regelsatz", "7"), &others).is_err());
    }

    #[test]
    fn a_rate_is_unique_whatever_it_is_written_like() {
        let others = [listed("Regelsatz", "19"), listed("Halb", "5.5")];

        assert!(check(&def("Normal", "19,0"), &others).is_err());
        assert!(check(&def("Normal", "5,50"), &others).is_err());
        assert!(check(&def("Normal", "0"), &others).is_err());
        assert!(check(&def("Normal", "x"), &others).is_err());
        assert!(check(&def("Normal", "16"), &others).is_ok());
    }
}
