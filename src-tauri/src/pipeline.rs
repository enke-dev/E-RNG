//! From invoice data to a validated ZUGFeRD PDF in a work directory:
//! rules check → CII XML → Typst PDF/A-3b → Mustang combine → Mustang validate.
//!
//! Nothing here decides whether the result may leave the work directory, that is the job of
//! `session::Sessions`, which only ever hands out files whose report was valid.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::draft::InputProblem;
use crate::invoice::{self, Finding, Invoice, ProfileKind};
use crate::package::{self, ValidationReport};
use crate::render::{self, Branding};
use crate::sidecar::Sidecars;

#[derive(Debug)]
pub struct Produced {
    pub pdf: PathBuf,
    pub xml: PathBuf,
    pub report: ValidationReport,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProduceError {
    /// The form holds something that is no invoice yet (a price that is no number)
    Input { problems: Vec<InputProblem> },
    /// The invoice breaks the profile's rules, nothing was generated
    Invalid { findings: Vec<Finding> },
    /// A tool failed or could not be run
    Failed { message: String },
}

impl From<anyhow::Error> for ProduceError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed {
            message: format!("{error:#}"),
        }
    }
}

/// File names are derived from the invoice number, so keep them harmless.
fn safe_name(number: &str) -> String {
    let name: String = number
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "invoice".into()
    } else {
        name
    }
}

pub fn produce(
    sidecars: &Sidecars,
    dir: &Path,
    invoice: &Invoice,
    profile: ProfileKind,
    branding: &Branding,
) -> Result<Produced, ProduceError> {
    let xml_name = match profile {
        ProfileKind::En16931 => "factur-x.xml",
        ProfileKind::XRechnung => "xrechnung.xml",
    };
    let pdf_name = format!(
        "{}.pdf",
        safe_name(invoice.number.as_deref().unwrap_or_default())
    );

    let xml = invoice::to_cii(invoice.clone(), profile)
        .map_err(|findings| ProduceError::Invalid { findings })?;
    std::fs::write(dir.join(xml_name), xml).map_err(anyhow::Error::from)?;

    render::render_pdf(sidecars, dir, invoice, branding)?;
    package::combine(sidecars, dir, "invoice.pdf", xml_name, profile, &pdf_name)?;
    let report = package::validate(sidecars, dir, &pdf_name)?;

    Ok(Produced {
        pdf: dir.join(pdf_name),
        xml: dir.join(xml_name),
        report,
    })
}

/// These run the real tools, so they are skipped unless asked for:
/// `bun run generate:sidecars && cargo test -- --ignored`
#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoice::golden::standard_invoice;
    use crate::package::Level;

    const NEEDS_SIDECARS: &str = "needs the sidecars: bun run generate:sidecars";

    fn branding() -> Branding {
        Branding {
            logo: None,
            footer: Some("Geschäftsführer: Max Muster · AG Köln HRB 12345".into()),
        }
    }

    #[test]
    fn file_names_stay_harmless() {
        assert_eq!(safe_name("RE-2026/001"), "RE-2026_001");
        assert_eq!(safe_name("../../etc"), "______etc");
        assert_eq!(safe_name(""), "invoice");
    }

    #[test]
    fn broken_invoices_are_refused_before_any_tool_runs() {
        let dir = tempfile::tempdir().unwrap();
        // the sidecar folder does not exist: reaching a tool would fail differently
        let sidecars = Sidecars::new(dir.path().join("nothing"));
        let broken = Invoice::default();

        let result = produce(
            &sidecars,
            dir.path(),
            &broken,
            ProfileKind::En16931,
            &branding(),
        );

        assert!(matches!(result, Err(ProduceError::Invalid { .. })));
    }

    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn en16931_invoice_becomes_a_valid_zugferd_pdf() {
        let _ = NEEDS_SIDECARS;
        let dir = tempfile::tempdir().unwrap();
        let invoice = standard_invoice(ProfileKind::En16931);

        let produced = produce(
            &Sidecars::in_repo(),
            dir.path(),
            &invoice,
            ProfileKind::En16931,
            &branding(),
        )
        .unwrap();

        let errors: Vec<_> = produced.report.errors().collect();
        assert!(produced.report.valid, "{errors:#?}");
        assert!(produced.pdf.exists() && produced.xml.exists());
    }

    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn xrechnung_invoice_becomes_a_valid_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let invoice = standard_invoice(ProfileKind::XRechnung);

        let produced = produce(
            &Sidecars::in_repo(),
            dir.path(),
            &invoice,
            ProfileKind::XRechnung,
            &branding(),
        )
        .unwrap();

        let errors: Vec<_> = produced.report.errors().collect();
        assert!(produced.report.valid, "{errors:#?}");
        assert!(produced.xml.ends_with("xrechnung.xml"));
    }

    /// With `RENDER_OUT` set to a directory the PDFs are kept there to look at them.
    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn every_golden_case_becomes_a_valid_pdf() {
        let sidecars = Sidecars::in_repo();
        for (name, invoice, profile) in crate::invoice::golden::cases() {
            let dir = tempfile::tempdir().unwrap();

            let produced = produce(&sidecars, dir.path(), &invoice, profile, &branding())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));

            let errors: Vec<_> = produced.report.errors().collect();
            assert!(produced.report.valid, "{name}: {errors:#?}");
            if let Some(out) = std::env::var_os("RENDER_OUT") {
                std::fs::copy(
                    &produced.pdf,
                    std::path::Path::new(&out).join(format!("{name}.pdf")),
                )
                .unwrap();
            }
        }
    }

    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn invoices_built_from_a_form_draft_pass_the_validator() {
        use crate::draft::{build_invoice, fixtures};

        for profile in [ProfileKind::En16931, ProfileKind::XRechnung] {
            let mut draft = fixtures::draft();
            draft.profile = profile;
            draft.buyer_reference = Some("991-01234-56".into());
            let invoice = build_invoice(&draft, &fixtures::settings(), "RE-2026-001").unwrap();
            let dir = tempfile::tempdir().unwrap();

            let produced = produce(
                &Sidecars::in_repo(),
                dir.path(),
                &invoice,
                profile,
                &branding(),
            )
            .unwrap();

            let errors: Vec<_> = produced.report.errors().collect();
            assert!(produced.report.valid, "{profile:?}: {errors:#?}");
        }
    }

    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn a_pdfa_without_the_facturx_xmp_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let sidecars = Sidecars::in_repo();
        let invoice = standard_invoice(ProfileKind::En16931);
        render::render_pdf(&sidecars, dir.path(), &invoice, &branding()).unwrap();

        // a PDF/A validator alone would pass this file, the ZUGFeRD check must not
        let report = package::validate(&sidecars, dir.path(), "invoice.pdf").unwrap();

        assert!(!report.valid);
        assert!(
            report.errors().any(|i| i.message.contains("XMP")),
            "{report:#?}"
        );
    }

    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn tampered_totals_are_rejected_by_the_validator() {
        let dir = tempfile::tempdir().unwrap();
        let sidecars = Sidecars::in_repo();
        let invoice = standard_invoice(ProfileKind::En16931);
        render::render_pdf(&sidecars, dir.path(), &invoice, &branding()).unwrap();
        let xml = invoice::to_cii(invoice, ProfileKind::En16931).unwrap();
        let tampered = regex_free_replace(&xml, "GrandTotalAmount", "1.00");
        std::fs::write(dir.path().join("tampered.xml"), tampered).unwrap();

        package::combine(
            &sidecars,
            dir.path(),
            "invoice.pdf",
            "tampered.xml",
            ProfileKind::En16931,
            "out.pdf",
        )
        .unwrap();
        let report = package::validate(&sidecars, dir.path(), "out.pdf").unwrap();

        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.level == Level::Error && i.code == "BR-CO-15"),
            "{report:#?}"
        );
    }

    /// Replaces the text of the first `<ram:tag currencyID=…>…</ram:tag>` element.
    fn regex_free_replace(xml: &str, tag: &str, value: &str) -> String {
        let open = format!("<ram:{tag}");
        let start = xml.find(&open).expect("element present");
        let content_start = start + xml[start..].find('>').unwrap() + 1;
        let content_end = content_start + xml[content_start..].find('<').unwrap();
        format!("{}{}{}", &xml[..content_start], value, &xml[content_end..])
    }
}
