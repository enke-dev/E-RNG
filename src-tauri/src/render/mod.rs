//! Renders the invoice with Typst: a PDF/A-3b for the document, SVG pages for the live preview. The ZUGFeRD parts (XML attachment, Factur-X XMP)
//! are not added here, that is what the packaging step does with the result.

mod view;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::invoice::Invoice;
use crate::sidecar::{self, Sidecars};

pub use view::Branding;

const TEMPLATE: &str = include_str!("invoice.typ");
const LOGO_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "svg"];

/// Writes template, data and logo into `dir`, everything Typst needs to compile `invoice.typ`.
fn stage(dir: &Path, invoice: &Invoice, branding: &Branding) -> Result<()> {
    let logo = branding
        .logo
        .as_deref()
        .map(|source| copy_logo(source, dir))
        .transpose()?;
    let data = view::build(invoice, branding, logo);

    std::fs::write(dir.join("invoice.typ"), TEMPLATE)?;
    std::fs::write(dir.join("data.json"), serde_json::to_vec_pretty(&data)?)?;
    Ok(())
}

fn compile(sidecars: &Sidecars, dir: &Path, options: &[&str], output: &str) -> Result<()> {
    // system fonts are ignored so the output does not depend on the machine it was made on
    let args: Vec<OsString> = ["compile", "--root", ".", "--ignore-system-fonts"]
        .iter()
        .chain(options)
        .chain(&["invoice.typ", output])
        .map(Into::into)
        .collect();
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
