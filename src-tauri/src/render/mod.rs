//! Renders the invoice with Typst: a PDF/A-3b for the document, SVG pages for the live preview. The `ZUGFeRD` parts (XML attachment, Factur-X XMP)
//! are not added here, that is what the packaging step does with the result.

mod compiler;
pub(crate) mod format;
mod invisible;
mod layouts;
mod parts;
mod view;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::invoice::Invoice;

use crate::layout::{self, CustomLayout, ImageFile};
pub use layouts::{all as layouts, LayoutInfo};
pub use view::Branding;

/// Where the fonts of an own layout are put next to the template
const FONTS_DIR: &str = "fonts";

/// The file Typst compiles
const MAIN: &str = "invoice.typ";

/// Writes template, data, fonts and pictures into `dir`, everything Typst needs to
/// compile `invoice.typ`.
fn stage(dir: &Path, invoice: &Invoice, branding: &Branding) -> Result<()> {
    // without an own layout the built-in one, made of the same parts
    let layout = branding.custom.clone().unwrap_or_else(layout::builtin);
    stage_fonts(&layout, dir)?;

    let mut data = view::build(invoice, branding, &layout.format);
    let mut staged = 0;
    let mut page = view::PageView {
        guides: branding.guides,
        marks: branding.marks,
        ..parts::page_view(&layout, &data, &mut |image| {
            staged += 1;
            stage_image(image, staged, dir)
        })?
    };
    // the marks of invisible characters belong to the guides, no other output gets them
    if branding.guides {
        invisible::reveal_page(&mut page);
    }
    data.page = page;

    std::fs::write(dir.join(MAIN), layouts::STANDARD_SOURCE)?;
    std::fs::write(dir.join("prelude.typ"), layouts::PRELUDE)?;
    std::fs::write(dir.join("data.json"), serde_json::to_vec_pretty(&data)?)?;
    Ok(())
}

/// Writes template, data and logo into `dir` and compiles `invoice.pdf` there. A document has
/// neither guides nor marks, whatever the branding asks for.
pub fn render_pdf(dir: &Path, invoice: &Invoice, branding: &Branding) -> Result<PathBuf> {
    let branding = Branding {
        guides: false,
        marks: false,
        ..branding.clone()
    };
    stage(dir, invoice, &branding)?;
    let pdf = dir.join("invoice.pdf");
    std::fs::write(&pdf, compiler::pdf(&compiler::compile(dir, MAIN)?)?)?;
    Ok(pdf)
}

/// The pages of the invoice as the template lays them out, one SVG each, for the live preview.
/// Same template and data as the PDF, only the output format differs (and no PDF/A).
pub fn render_preview(dir: &Path, invoice: &Invoice, branding: &Branding) -> Result<Vec<String>> {
    stage(dir, invoice, branding)?;
    Ok(compiler::svg(&compiler::compile(dir, MAIN)?))
}

/// Where a part of the layout is on a page, in millimetres from its top left corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartBox {
    /// From 1
    pub page: u32,
    /// On the setup of the first page, else on the one of the others
    pub first: bool,
    /// The place of the part in the list of its setup
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The preview with the boxes of the parts, for the editor of a layout: what is under the pointer
/// in the sample is the part to edit. The template hands the boxes out as metadata, the compiled
/// document is asked for them.
pub fn render_sample(
    dir: &Path,
    invoice: &Invoice,
    branding: &Branding,
) -> Result<(Vec<String>, Vec<PartBox>)> {
    let branding = Branding {
        marks: true,
        ..branding.clone()
    };
    stage(dir, invoice, &branding)?;
    let document = compiler::compile(dir, MAIN)?;
    let boxes = compiler::metadata(&document, "part-box")?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<PartBox>, _>>()
        .context("the part boxes are not readable")?;
    Ok((compiler::svg(&document), boxes))
}

fn stage_fonts(custom: &CustomLayout, dir: &Path) -> Result<()> {
    if custom.fonts.is_empty() {
        return Ok(());
    }
    let target = dir.join(FONTS_DIR);
    std::fs::create_dir_all(&target)?;
    custom.fonts.iter().try_for_each(|font| {
        let name = font
            .path
            .file_name()
            .context("a font without a file name")?;
        std::fs::copy(&font.path, target.join(name))
            .map(drop)
            .with_context(|| format!("cannot read the font {}", font.path.display()))
    })
}

/// Puts a picture of a part next to the template, returns the name it has there.
fn stage_image(image: &ImageFile, number: usize, dir: &Path) -> Result<String> {
    let extension = image
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let name = format!("image-{number}.{extension}");
    std::fs::copy(&image.path, dir.join(&name))
        .with_context(|| format!("cannot read the image {}", image.path.display()))?;
    Ok(name)
}

/// The first page of the Typst file `main` in `dir` as SVG, for the tests that make letterheads
#[cfg(test)]
pub(crate) fn svg_of(dir: &Path, main: &str) -> Result<String> {
    compiler::svg(&compiler::compile(dir, main)?)
        .into_iter()
        .next()
        .context("a document without pages")
}

/// The first page of the Typst file `main` in `dir` as PNG, to look at the SVG of a test
#[cfg(test)]
pub(crate) fn png_of(dir: &Path, main: &str, ppi: f64) -> Result<Vec<u8>> {
    compiler::png(&compiler::compile(dir, main)?, ppi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{build_invoice, fixtures};
    use crate::layout::PartContent;
    use serde_json::Value;

    /// A layout with a text of the characters a paste brings along, and a page number
    fn pasted() -> CustomLayout {
        let mut layout = layout::builtin();
        let pasted = layout
            .page
            .parts
            .iter_mut()
            .find_map(|part| match &mut part.content {
                PartContent::Text { text, .. } => Some(text),
                _ => None,
            });
        *pasted.expect("a text part") = "Tel.\t0221\u{00A0}12\u{200B}3 4\nzweite Zeile".into();
        layout
    }

    /// `data.json` of the staged invoice
    fn staged(guides: bool) -> Value {
        let settings = fixtures::settings();
        let draft = fixtures::draft();
        let invoice = build_invoice(&draft, &settings, "RE-1").unwrap();
        let branding = Branding {
            guides,
            custom: Some(pasted()),
            ..Branding::for_document(crate::model::Kind::Invoice, &draft, &settings, &[])
        };
        let dir = tempfile::tempdir().unwrap();
        stage(dir.path(), &invoice, &branding).unwrap();
        serde_json::from_slice(&std::fs::read(dir.path().join("data.json")).unwrap()).unwrap()
    }

    /// The text of every line of every text part, columns divided by a tab: what a mark must not change
    fn texts(data: &Value) -> Vec<String> {
        data["page"]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|part| part["kind"] == "text")
            .flat_map(|part| part["lines"].as_array().unwrap())
            .map(|line| {
                line.as_array()
                    .unwrap()
                    .iter()
                    .map(|column| {
                        column
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|run| run["text"].as_str().unwrap())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect()
    }

    #[test]
    fn marks_of_invisible_characters_are_made_for_the_guides_only() {
        let plain = staged(false);
        let guided = staged(true);

        // without the guides nothing says mark: the template has nothing to draw
        let plain_json = plain.to_string();
        assert!(!plain_json.contains(r#""mark""#), "a mark without guides");
        assert!(
            !plain_json.contains(r#""marked""#),
            "a marked text without guides"
        );
        assert_eq!(plain["page"]["guides"], false);

        // with them the same text is there, now with a mark for each character
        let guided_json = guided.to_string();
        assert_eq!(guided["page"]["guides"], true);
        for expected in [
            r#""kind":"space""#,
            r#""kind":"column""#,
            r#""kind":"line-end""#,
            r#""label":"NBSP""#,
            r#""label":"ZWSP""#,
            r#""marked""#,
        ] {
            assert!(guided_json.contains(expected), "{expected} is missing");
        }
        assert_eq!(texts(&plain), texts(&guided));
        assert!(texts(&plain)
            .iter()
            .any(|line| line == "Tel.\t0221\u{00A0}12\u{200B}3 4"));
    }
}
