//! Renders the invoice to a PDF/A-3b with Typst. The ZUGFeRD parts (XML attachment, Factur-X XMP)
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

/// Writes template, data and logo into `dir` and compiles `invoice.pdf` there.
pub fn render_pdf(
    sidecars: &Sidecars,
    dir: &Path,
    invoice: &Invoice,
    branding: &Branding,
) -> Result<PathBuf> {
    let logo = branding
        .logo
        .as_deref()
        .map(|source| copy_logo(source, dir))
        .transpose()?;
    let data = view::build(invoice, branding, logo);

    std::fs::write(dir.join("invoice.typ"), TEMPLATE)?;
    std::fs::write(dir.join("data.json"), serde_json::to_vec_pretty(&data)?)?;

    // system fonts are ignored so the output does not depend on the machine it was made on
    let args: Vec<OsString> = [
        "compile",
        "--root",
        ".",
        "--pdf-standard",
        "a-3b",
        "--ignore-system-fonts",
        "invoice.typ",
        "invoice.pdf",
    ]
    .iter()
    .map(Into::into)
    .collect();
    let out = sidecar::run(&sidecars.typst(), &args, dir, Duration::from_secs(60))?;
    if out.code != Some(0) {
        bail!("typst failed: {}", out.stderr.trim());
    }
    Ok(dir.join("invoice.pdf"))
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
