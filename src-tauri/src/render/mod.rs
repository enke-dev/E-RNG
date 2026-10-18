//! Renders the invoice with Typst: a PDF/A-3b for the document, SVG pages for the live preview. The ZUGFeRD parts (XML attachment, Factur-X XMP)
//! are not added here, that is what the packaging step does with the result.

mod layouts;
mod view;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::invoice::Invoice;
use crate::sidecar::{self, Sidecars};

use crate::model::{Background, CustomLayout};
pub use layouts::{all as layouts, LayoutInfo};
pub use view::Branding;

/// Where the fonts of an own layout are put next to the template
const FONTS_DIR: &str = "fonts";

const LOGO_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "svg"];

/// Writes template, data and logo into `dir`, everything Typst needs to compile `invoice.typ`.
fn stage(dir: &Path, invoice: &Invoice, branding: &Branding) -> Result<()> {
    let logo = branding
        .logo
        .as_deref()
        .map(|source| copy_logo(source, dir))
        .transpose()?;
    let files = branding
        .custom
        .as_ref()
        .and_then(|custom| custom.background.as_ref())
        .map(|background| stage_background(background, dir))
        .transpose()?;
    if let Some(custom) = &branding.custom {
        stage_fonts(custom, dir)?;
    }
    let data = view::build(
        invoice,
        branding,
        logo,
        view::PageView {
            guides: branding.guides,
            ..view::PageView::of(branding.custom.as_ref(), files)
        },
    );

    std::fs::write(dir.join("invoice.typ"), layouts::STANDARD_SOURCE)?;
    std::fs::write(dir.join("prelude.typ"), layouts::PRELUDE)?;
    std::fs::write(dir.join("data.json"), serde_json::to_vec_pretty(&data)?)?;
    Ok(())
}

fn compile(sidecars: &Sidecars, dir: &Path, options: &[&str], output: &str) -> Result<()> {
    // system fonts are ignored so the output does not depend on the machine it was made on
    let mut args: Vec<OsString> = ["compile", "--root", ".", "--ignore-system-fonts"]
        .iter()
        .map(Into::into)
        .collect();
    // the fonts of the user's own layout, besides those Typst brings itself
    if dir.join(FONTS_DIR).is_dir() {
        args.push("--font-path".into());
        args.push(dir.join(FONTS_DIR).into());
    }
    args.extend(options.iter().map(Into::into));
    args.extend(["invoice.typ", output].iter().map(Into::into));
    let out = sidecar::run(&sidecars.typst(), &args, dir, Duration::from_secs(60))?;
    if out.code != Some(0) {
        bail!("typst failed: {}", out.stderr.trim());
    }
    Ok(())
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

    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("page-") && n.ends_with(".svg"))
        })
        .collect();
    // `{0p}` pads the number, so the names sort like the pages
    files.sort();
    files
        .iter()
        .map(|file| std::fs::read_to_string(file).map_err(Into::into))
        .collect()
}

/// Puts the stationery next to the template, returns the names the template finds it under.
fn stage_background(background: &Background, dir: &Path) -> Result<(String, Option<String>)> {
    let copy = |source: &Path, name: &str| -> Result<String> {
        std::fs::copy(source, dir.join(name))
            .with_context(|| format!("cannot read the stationery {}", source.display()))?;
        Ok(name.to_owned())
    };
    Ok((
        copy(&background.first, "background-1.svg")?,
        background
            .rest
            .as_deref()
            .map(|rest| copy(rest, "background-2.svg"))
            .transpose()?,
    ))
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

fn copy_logo(source: &Path, dir: &Path) -> Result<String> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| LOGO_EXTENSIONS.contains(&e.as_str()))
        .context("the logo must be a png, jpg or svg file")?;
    let name = format!("logo.{extension}");
    std::fs::copy(source, dir.join(&name))
        .with_context(|| format!("cannot read the logo {}", source.display()))?;
    Ok(name)
}
