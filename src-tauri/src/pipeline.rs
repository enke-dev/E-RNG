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
            ..Branding::default()
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

    fn with_letter() -> Branding {
        Branding {
            intro: Some("Sehr geehrte Damen und Herren,\nhiermit stelle ich folgende Positionen in Rechnung.".into()),
            closing: Some("Vielen Dank für das entgegengebrachte Vertrauen.".into()),
            ..branding()
        }
    }

    /// With `RENDER_OUT` set to a directory the PDFs are kept there to look at them.
    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn every_golden_case_becomes_a_valid_pdf() {
        let sidecars = Sidecars::in_repo();
        let layout = "standard";
        for (name, invoice, profile) in crate::invoice::golden::cases() {
            let dir = tempfile::tempdir().unwrap();

            let produced = produce(&sidecars, dir.path(), &invoice, profile, &with_letter())
                .unwrap_or_else(|e| panic!("{layout}/{name}: {e:?}"));

            let errors: Vec<_> = produced.report.errors().collect();
            assert!(produced.report.valid, "{layout}/{name}: {errors:#?}");
            if let Some(out) = std::env::var_os("RENDER_OUT") {
                let file = std::path::Path::new(&out).join(format!("{layout}-{name}.pdf"));
                std::fs::copy(&produced.pdf, file).unwrap();
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
    fn the_preview_renders_one_svg_per_page() {
        let sidecars = Sidecars::in_repo();
        let cases = crate::invoice::golden::cases();
        let layout = "standard";
        let pages = |name: &str| {
            let (_, invoice, _) = cases.iter().find(|(n, ..)| *n == name).unwrap();
            let dir = tempfile::tempdir().unwrap();
            render::render_preview(&sidecars, dir.path(), invoice, &with_letter()).unwrap()
        };

        let short = pages("standard");
        let long = pages("many-lines");

        assert_eq!(short.len(), 1, "{layout}");
        assert!(long.len() >= 3, "{layout}: {} pages", long.len());
        assert!(
            short.iter().chain(&long).all(|svg| svg.contains("<svg")),
            "{layout}"
        );

        // with `RENDER_OUT` the pages are kept to look at
        if let Some(out) = std::env::var_os("RENDER_OUT") {
            for (name, pages) in [("short", &short), ("long", &long)] {
                for (i, svg) in pages.iter().enumerate() {
                    let file = format!("preview-{layout}-{name}-{}.svg", i + 1);
                    std::fs::write(std::path::Path::new(&out).join(file), svg).unwrap();
                }
            }
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

    /// Stationery from a PDF (as SVG, Typst cannot embed it in PDF/A) and an uploaded font:
    /// the result must still validate. `RENDER_OUT` keeps the PDFs.
    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn an_own_layout_with_stationery_and_font_is_a_valid_pdf() {
        use crate::model::{CustomLayout, Insets};
        let sidecars = Sidecars::in_repo();
        let work = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();

        // a two page letterhead: a blue band on the first page, a thin line on the next
        std::fs::write(
            work.path().join("paper.typ"),
            "#set page(paper: \"a4\", margin: 0pt)\n\
             #place(top, rect(width: 100%, height: 4cm, fill: rgb(\"#1d4ed8\")))\n\
             #place(bottom, rect(width: 100%, height: 1.5cm, fill: luma(225)))\n\
             #pagebreak()\n\
             #place(top, rect(width: 100%, height: 1cm, fill: luma(225)))",
        )
        .unwrap();
        let out = crate::sidecar::run(
            &sidecars.typst(),
            &["compile".into(), "paper.typ".into(), "paper.pdf".into()],
            work.path(),
            std::time::Duration::from_secs(60),
        )
        .unwrap();
        assert_eq!(out.code, Some(0), "{}", out.stderr);

        let background =
            crate::layout_files::import_background(&work.path().join("paper.pdf"), files.path())
                .unwrap();
        assert!(background.rest.is_some());
        let font = crate::layout_files::import_font(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/OpenSans.ttf"),
            &files.path().join("fonts"),
        )
        .unwrap();
        assert_eq!(font.family, "Open Sans");

        {
            let base = "standard";
            for (name, invoice, profile) in crate::invoice::golden::cases() {
                let dir = tempfile::tempdir().unwrap();
                let branding = Branding {
                    custom: Some(CustomLayout {
                        id: Some(1),
                        name: "Briefpapier".into(),
                        background: Some(background.clone()),
                        margins: Insets {
                            top: 30.0,
                            right: 20.0,
                            bottom: 30.0,
                            left: 25.0,
                        },
                        first_margins: Some(Insets {
                            top: 70.0,
                            right: 35.0,
                            bottom: 40.0,
                            left: 40.0,
                        }),
                        fonts: vec![font.clone()],
                        archived: false,
                    }),
                    ..with_letter()
                };
                let produced =
                    produce(&sidecars, dir.path(), &invoice, profile, &branding).unwrap();
                let errors: Vec<_> = produced.report.errors().collect();
                assert!(produced.report.valid, "{base}/{name}: {errors:#?}");
                if let Ok(keep) = std::env::var("RENDER_OUT") {
                    let guided = Branding {
                        guides: true,
                        ..branding.clone()
                    };
                    let pages =
                        render::render_preview(&sidecars, dir.path(), &invoice, &guided).unwrap();
                    pages.iter().enumerate().for_each(|(i, svg)| {
                        std::fs::write(format!("{keep}/own-{base}-{name}-{}.svg", i + 1), svg)
                            .unwrap()
                    });
                    std::fs::create_dir_all(&keep).unwrap();
                    std::fs::copy(&produced.pdf, format!("{keep}/own-{base}-{name}.pdf")).unwrap();
                }
            }
        }
    }
}
