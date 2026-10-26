//! Renders the invoice with Typst: a PDF/A-3b for the document, SVG pages for the live preview. The `ZUGFeRD` parts (XML attachment, Factur-X XMP)
//! are not added here, that is what the packaging step does with the result.

pub(crate) mod format;
mod layouts;
mod parts;
mod view;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::invoice::Invoice;
use crate::sidecar::{self, Sidecars};

use crate::layout::{self, CustomLayout, ImageFile};
pub use layouts::{all as layouts, LayoutInfo};
pub use view::Branding;

/// Where the fonts of an own layout are put next to the template
const FONTS_DIR: &str = "fonts";

/// Writes template, data, fonts and pictures into `dir`, everything Typst needs to
/// compile `invoice.typ`.
fn stage(dir: &Path, invoice: &Invoice, branding: &Branding) -> Result<()> {
    // without an own layout the built-in one, made of the same parts
    let layout = branding.custom.clone().unwrap_or_else(layout::builtin);
    stage_fonts(&layout, dir)?;

    let mut data = view::build(invoice, branding, &layout.format);
    let mut staged = 0;
    data.page = view::PageView {
        guides: branding.guides,
        marks: branding.marks,
        ..parts::page_view(&layout, &data, &mut |image| {
            staged += 1;
            stage_image(image, staged, dir)
        })?
    };

    std::fs::write(dir.join("invoice.typ"), layouts::STANDARD_SOURCE)?;
    std::fs::write(dir.join("prelude.typ"), layouts::PRELUDE)?;
    std::fs::write(dir.join("data.json"), serde_json::to_vec_pretty(&data)?)?;
    Ok(())
}

/// What every Typst command needs to see the same world: the folder as root, the fonts. System
/// fonts are ignored so the output does not depend on the machine it was made on.
fn world_args(dir: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["--root", ".", "--ignore-system-fonts"]
        .iter()
        .map(Into::into)
        .collect();
    // the fonts of the user's own layout, besides those Typst brings itself
    if dir.join(FONTS_DIR).is_dir() {
        args.push("--font-path".into());
        args.push(dir.join(FONTS_DIR).into());
    }
    args
}

fn typst(sidecars: &Sidecars, dir: &Path, args: &[OsString]) -> Result<String> {
    let out = sidecar::run(&sidecars.typst(), args, dir, Duration::from_secs(60))?;
    if out.code != Some(0) {
        bail!("typst failed: {}", out.stderr.trim());
    }
    Ok(out.stdout)
}

fn compile(sidecars: &Sidecars, dir: &Path, options: &[&str], output: &str) -> Result<()> {
    let mut args: Vec<OsString> = vec!["compile".into()];
    args.extend(world_args(dir));
    args.extend(options.iter().map(Into::into));
    args.extend(["invoice.typ", output].iter().map(Into::into));
    typst(sidecars, dir, &args).map(drop)
}

/// Writes template, data and logo into `dir` and compiles `invoice.pdf` there.
pub fn render_pdf(
    sidecars: &Sidecars,
    dir: &Path,
    invoice: &Invoice,
    branding: &Branding,
) -> Result<PathBuf> {
    stage(dir, invoice, branding)?;
    compile(sidecars, dir, &["--pdf-standard", "a-3b"], "invoice.pdf")?;
    Ok(dir.join("invoice.pdf"))
}

/// The pages of the invoice as the template lays them out, one SVG each, for the live preview.
/// Same template and data as the PDF, only the output format differs (and no PDF/A).
pub fn render_preview(
    sidecars: &Sidecars,
    dir: &Path,
    invoice: &Invoice,
    branding: &Branding,
) -> Result<Vec<String>> {
    stage(dir, invoice, branding)?;
    compile(sidecars, dir, &["--format", "svg"], "page-{0p}.svg")?;
    read_pages(dir)
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
/// in the sample is the part to edit. The template hands the boxes out, a query reads them.
pub fn render_sample(
    sidecars: &Sidecars,
    dir: &Path,
    invoice: &Invoice,
    branding: &Branding,
) -> Result<(Vec<String>, Vec<PartBox>)> {
    let branding = Branding {
        marks: true,
        ..branding.clone()
    };
    stage(dir, invoice, &branding)?;
    compile(sidecars, dir, &["--format", "svg"], "page-{0p}.svg")?;
    let pages = read_pages(dir)?;

    let mut args: Vec<OsString> = vec!["query".into()];
    args.extend(world_args(dir));
    args.extend(["invoice.typ", "<part-box>", "--field", "value"].map(OsString::from));
    let boxes = serde_json::from_str(&typst(sidecars, dir, &args)?)
        .context("the part boxes are not readable")?;
    Ok((pages, boxes))
}

fn has_svg_extension(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

fn read_pages(dir: &Path) -> Result<Vec<String>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("page-") && has_svg_extension(n))
        })
        .collect();
    // `{0p}` pads the number, so the names sort like the pages
    files.sort();
    files
        .iter()
        .map(|file| std::fs::read_to_string(file).map_err(Into::into))
        .collect()
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
