//! German texts for the rule findings. The rule library speaks the standards' English (the
//! KoSIT ones partly German), the user of this app should not have to.
//!
//! Only rules the form can actually trigger are covered, each text says where to fix the problem.
//! Anything else keeps its original wording, and the original always stays available as `hint`.

use en16931::validation::Finding;

const TEXTS: &[(&str, &str)] = &[
    ("BR-06", "Der Name des Unternehmens fehlt (Einstellungen)."),
    ("BR-07", "Der Kunde hat keinen Namen."),
    ("BR-27", "Der Preis einer Position darf nicht negativ sein."),
    ("BR-50", "Die IBAN fehlt (Einstellungen, Bankverbindung)."),
    ("BR-61", "Die IBAN fehlt (Einstellungen, Bankverbindung)."),
    (
        "BR-CO-09",
        "Die USt-IdNr. muss mit einem Ländercode beginnen, z. B. DE123456789 (Einstellungen oder Kunde).",
    ),
    (
        "BR-CO-26",
        "Zur Identifikation des Unternehmens fehlt die USt-IdNr. oder die Steuernummer (Einstellungen).",
    ),
    (
        "BR-S-02",
        "Rechnungen mit Umsatzsteuer brauchen die USt-IdNr. oder die Steuernummer des Unternehmens (Einstellungen).",
    ),
    (
        "BR-DE-2",
        "Für XRechnung ist ein Ansprechpartner des Unternehmens erforderlich (Einstellungen).",
    ),
    (
        "BR-DE-3",
        "Der Ort des Unternehmens fehlt (Einstellungen).",
    ),
    (
        "BR-DE-4",
        "Die PLZ des Unternehmens fehlt (Einstellungen).",
    ),
    (
        "BR-DE-5",
        "Für XRechnung fehlt der Name des Ansprechpartners (Einstellungen).",
    ),
    (
        "BR-DE-6",
        "Für XRechnung fehlt die Telefonnummer des Ansprechpartners (Einstellungen).",
    ),
    (
        "BR-DE-7",
        "Für XRechnung fehlt die E-Mail-Adresse des Ansprechpartners (Einstellungen).",
    ),
    ("BR-DE-8", "Der Ort des Kunden fehlt."),
    ("BR-DE-9", "Die PLZ des Kunden fehlt."),
    (
        "BR-DE-15",
        "Für XRechnung ist die Leitweg-ID (Käuferreferenz) erforderlich.",
    ),
    (
        "BR-DE-16",
        "Das Unternehmen braucht für Rechnungen mit Umsatzsteuer eine USt-IdNr. oder eine Steuernummer (Einstellungen).",
    ),
    ("BR-DE-19", "Die IBAN scheint ungültig zu sein (Einstellungen)."),
    (
        "PEPPOL-EN16931-R010",
        "Für XRechnung fehlt die E-Mail-Adresse des Kunden.",
    ),
    (
        "PEPPOL-EN16931-R020",
        "Für XRechnung fehlt die E-Mail-Adresse des Unternehmens (Einstellungen).",
    ),
];

fn text(rule: &str) -> Option<&'static str> {
    TEXTS
        .iter()
        .find(|(id, _)| *id == rule)
        .map(|(_, text)| *text)
}

/// The finding with a German message where one is known, the original wording moves to `hint`.
pub fn localize(finding: Finding) -> Finding {
    match text(&finding.rule) {
        Some(german) => Finding {
            hint: Some(finding.message.clone()),
            message: german.to_owned(),
            ..finding
        },
        None => finding,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::draft::{build_invoice, fixtures};
    use crate::invoice::{validate, ProfileKind};
    use crate::model::{InvoiceDraft, Settings};

    #[test]
    fn known_rules_get_german_and_keep_the_original() {
        let original = Finding {
            rule: "BR-DE-15".into(),
            severity: en16931::Severity::Fatal,
            path: en16931::Path::group(en16931::Group::Document),
            message: "Buyer reference (BT-10) shall be present".into(),
            detail: None,
            hint: None,
        };

        let german = localize(original);

        assert!(german.message.contains("Leitweg-ID"));
        assert_eq!(
            german.hint.as_deref(),
            Some("Buyer reference (BT-10) shall be present")
        );
    }

    #[test]
    fn unknown_rules_are_left_alone() {
        let finding = Finding {
            rule: "BR-XX-99".into(),
            severity: en16931::Severity::Fatal,
            path: en16931::Path::group(en16931::Group::Document),
            message: "original".into(),
            detail: None,
            hint: None,
        };

        assert_eq!(localize(finding.clone()), finding);
    }

    /// Breaks the form field by field and collects every rule that fires. A rule without a German
    /// text fails here, which is also what flags new rules after a crate upgrade.
    #[test]
    fn every_rule_the_form_can_trigger_has_a_german_text() {
        type Tweak = fn(&mut InvoiceDraft, &mut Settings);
        let tweaks: &[Tweak] = &[
            |_, s| s.seller.name = String::new(),
            |_, s| s.seller.address.city = String::new(),
            |_, s| s.seller.address.post_code = String::new(),
            |_, s| {
                s.seller.vat_id = None;
                s.seller.tax_number = None;
            },
            |_, s| s.seller.vat_id = Some("XX123".into()),
            |_, s| {
                s.seller.contact_name = String::new();
                s.seller.phone = String::new();
                s.seller.email = String::new();
            },
            |_, s| s.seller.phone = String::new(),
            |_, s| s.seller.email = String::new(),
            |_, s| s.seller.iban = String::new(),
            |_, s| s.seller.iban = "DE00".into(),
            |d, _| d.customer.name = String::new(),
            |d, _| d.customer.address.city = String::new(),
            |d, _| d.customer.address.post_code = String::new(),
            |d, _| d.customer.email = None,
            |d, _| d.customer.vat_id = Some("123".into()),
            |d, _| d.buyer_reference = None,
            |d, _| d.positions[0].price = "-5".into(),
        ];

        let mut missing = BTreeSet::new();
        for profile in [ProfileKind::En16931, ProfileKind::XRechnung] {
            for tweak in tweaks {
                let mut draft = fixtures::draft();
                draft.profile = profile;
                let mut settings = fixtures::settings();
                tweak(&mut draft, &mut settings);
                let Ok(invoice) = build_invoice(&draft, &settings, "RE-1") else {
                    continue;
                };
                missing.extend(
                    validate(&invoice, profile)
                        .into_iter()
                        .filter(|f| f.hint.is_none())
                        .map(|f| f.rule),
                );
            }
        }

        assert!(missing.is_empty(), "no German text for {missing:?}");
    }
}
