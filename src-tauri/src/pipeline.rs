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
            legal: Some("Geschäftsführer: Max Muster · AG Köln HRB 12345".into()),
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

    /// Without a rate and without payment days: valid for the validator, and the document says
    /// neither (`RENDER_OUT` keeps the PDF).
    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn an_invoice_without_vat_and_payment_term_is_a_valid_pdf() {
        use crate::draft::{build_invoice, fixtures};

        let free = crate::model::Settings {
            tax_rate: String::new(),
            payment_days: None,
            ..fixtures::settings()
        };
        let draft = fixtures::draft();
        let invoice = build_invoice(&draft, &free, "RE-2026-001").unwrap();
        let branding = Branding::for_draft(&draft, &free, &[]);
        let dir = tempfile::tempdir().unwrap();

        let produced = produce(
            &Sidecars::in_repo(),
            dir.path(),
            &invoice,
            ProfileKind::En16931,
            &branding,
        )
        .unwrap();

        let errors: Vec<_> = produced.report.errors().collect();
        assert!(produced.report.valid, "{errors:#?}");
        if let Some(out) = std::env::var_os("RENDER_OUT") {
            std::fs::copy(
                &produced.pdf,
                std::path::Path::new(&out).join("without-vat.pdf"),
            )
            .unwrap();
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

    /// The built-in layout with a full page picture as letterhead on every page (a different one
    /// on the first), an uploaded font, and a frame and parts of its own on the first page.
    fn own_layout(
        letterheads: [&crate::layout::ImageFile; 2],
        font: &crate::layout::FontFile,
    ) -> crate::layout::CustomLayout {
        use crate::layout::{builtin, Anchor, Insets, Part, PartContent};
        let full_page = |image: &crate::layout::ImageFile| Part {
            anchor: Anchor::TopLeft,
            x: 0.0,
            y: 0.0,
            width: Some(210.0),
            content: PartContent::Image {
                image: Some(image.clone()),
            },
        };
        let mut layout = crate::layout::CustomLayout {
            id: Some(1),
            name: "Briefpapier".into(),
            fonts: vec![font.clone()],
            ..builtin()
        };
        layout.page.margins = Insets {
            top: 30.0,
            right: 20.0,
            bottom: 40.0,
            left: 25.0,
        };
        layout.page.parts.push(full_page(letterheads[1]));
        if let Some(first) = layout.first.as_mut() {
            first.margins = Insets {
                top: 90.0,
                right: 35.0,
                bottom: 40.0,
                left: 40.0,
            };
            first.parts.push(full_page(letterheads[0]));
        }
        layout
    }

    /// Letterheads as full page pictures (SVG, made with Typst) and an uploaded font: the result
    /// must still validate. `RENDER_OUT` keeps the PDFs.
    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn an_own_layout_with_letterheads_and_font_is_a_valid_pdf() {
        let sidecars = Sidecars::in_repo();
        let work = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();

        // two letterheads: a blue band and a grey footer strip, and a thin line for the other pages
        let letterhead = |name: &str, content: &str| {
            std::fs::write(
                work.path().join(format!("{name}.typ")),
                format!("#set page(paper: \"a4\", margin: 0pt)\n{content}"),
            )
            .unwrap();
            let out = crate::sidecar::run(
                &sidecars.typst(),
                &[
                    "compile".into(),
                    "--format".into(),
                    "svg".into(),
                    format!("{name}.typ").into(),
                    format!("{name}.svg").into(),
                ],
                work.path(),
                std::time::Duration::from_secs(60),
            )
            .unwrap();
            assert_eq!(out.code, Some(0), "{}", out.stderr);
            crate::layout_files::import_image(
                &work.path().join(format!("{name}.svg")),
                files.path(),
            )
            .unwrap()
        };
        let first = letterhead(
            "first",
            "#place(top, rect(width: 100%, height: 4cm, fill: rgb(\"#1d4ed8\")))\n\
             #place(bottom, rect(width: 100%, height: 1.5cm, fill: luma(225)))",
        );
        let rest = letterhead(
            "rest",
            "#place(top, rect(width: 100%, height: 1cm, fill: luma(225)))",
        );
        assert!(first.vector && rest.vector);
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
                    custom: Some(own_layout([&first, &rest], &font)),
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

    /// A picture part (too few pixels for its size, so the sample marks it), guides and marks in the
    /// preview, and the invoice with that layout still validates. `RENDER_OUT` keeps the pages.
    #[test]
    #[ignore = "needs the sidecars: bun run generate:sidecars"]
    fn an_image_part_is_placed_marked_when_blurry_and_valid_in_the_pdf() {
        use crate::layout::{builtin, Anchor, ImageFile, Part, PartContent};
        let sidecars = Sidecars::in_repo();
        let dir = tempfile::tempdir().unwrap();
        // a one pixel png: far too few pixels for 40 mm
        let bytes = base64_decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC",
        );
        let path = dir.path().join("pixel.png");
        std::fs::write(&path, bytes).unwrap();

        let mut layout = builtin();
        layout.first.as_mut().unwrap().parts.push(Part {
            anchor: Anchor::TopLeft,
            x: 25.0,
            y: 12.0,
            width: Some(40.0),
            content: PartContent::Image {
                image: Some(ImageFile {
                    name: "pixel.png".into(),
                    path,
                    width_px: 1,
                    height_px: 1,
                    vector: false,
                }),
            },
        });
        let branding = Branding {
            custom: Some(layout),
            ..with_letter()
        };
        let (_, invoice, profile) = crate::invoice::golden::cases().remove(0);

        let produced = produce(&sidecars, dir.path(), &invoice, profile, &branding).unwrap();
        assert!(
            produced.report.valid,
            "{:#?}",
            produced.report.errors().collect::<Vec<_>>()
        );

        let sample = Branding {
            guides: true,
            marks: true,
            ..branding
        };
        let (pages, boxes) =
            render::render_sample(&sidecars, dir.path(), &invoice, &sample).unwrap();
        // every part of the first page is somewhere on page 1, the infos end at the right margin
        let first_page: Vec<_> = boxes.iter().filter(|b| b.page == 1 && b.first).collect();
        assert!(first_page.len() >= 8, "{boxes:#?}");
        assert!(first_page.iter().all(|b| b.w > 0.0));
        let infos = first_page.iter().find(|b| b.index == 8).unwrap();
        assert!((infos.x + infos.w - 190.0).abs() < 0.5, "{infos:?}");
        assert!((infos.y - 50.0).abs() < 0.5, "{infos:?}");
        if let Ok(keep) = std::env::var("RENDER_OUT") {
            std::fs::create_dir_all(&keep).unwrap();
            std::fs::write(format!("{keep}/image-part.svg"), &pages[0]).unwrap();
        }
    }

    /// Enough of base64 for a fixture.
    fn base64_decode(text: &str) -> Vec<u8> {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let (mut buffer, mut bits) = (0u32, 0);
        for c in text.bytes().filter(|c| *c != b'=') {
            buffer = buffer << 6
                | u32::try_from(ALPHABET.iter().position(|a| *a == c).unwrap()).unwrap();
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(u8::try_from(buffer >> bits & 0xff).unwrap());
            }
        }
        out
    }
}
