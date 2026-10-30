//! The originals of invoices: the PDF and the XML that were validated when an invoice was created
//! stay in the app's data folder, with a hash each. What is handed out later is these files, so what
//! was sent is what is archived. An offer keeps its PDF only, in a folder of its own.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::model::{InvoiceFiles, Kind, StoredFile};
use crate::pipeline::{safe_name, Produced};

/// Where the originals of a context's documents of a kind live, relative to the data folder. Offers
/// are kept apart so equal names of the two kinds cannot overwrite each other.
fn folder(kind: Kind, context_id: i64) -> PathBuf {
    PathBuf::from(match kind {
        Kind::Invoice => "invoices",
        Kind::Offer => "offers",
    })
    .join(context_id.to_string())
}

fn sha256(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Copies one produced file into the folder and notes its hash
fn keep(data: &Path, relative: PathBuf, from: &Path) -> Result<StoredFile> {
    let target = data.join(&relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, &target).with_context(|| format!("cannot keep {}", from.display()))?;
    Ok(StoredFile {
        // with `/` on every system, a database or backup moves between them
        path: relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        sha256: sha256(&target)?,
    })
}

/// Keeps the files of a created invoice. On failure nothing stays behind.
pub fn store(
    data: &Path,
    context_id: i64,
    number: &str,
    produced: &Produced,
    regenerated: bool,
) -> Result<InvoiceFiles> {
    let name = safe_name(number);
    let folder = folder(Kind::Invoice, context_id);
    let pdf = keep(data, folder.join(format!("{name}.pdf")), &produced.pdf)?;
    let xml = match keep(data, folder.join(format!("{name}.xml")), &produced.xml) {
        Ok(xml) => xml,
        Err(error) => {
            let _ = std::fs::remove_file(data.join(&pdf.path));
            return Err(error);
        }
    };
    Ok(InvoiceFiles {
        pdf,
        xml: Some(xml),
        regenerated,
    })
}

/// Keeps the PDF of a created offer, its only file
pub fn store_pdf(
    data: &Path,
    kind: Kind,
    context_id: i64,
    number: &str,
    pdf: &Path,
) -> Result<InvoiceFiles> {
    let relative = folder(kind, context_id).join(format!("{}.pdf", safe_name(number)));
    Ok(InvoiceFiles {
        pdf: keep(data, relative, pdf)?,
        xml: None,
        regenerated: false,
    })
}

/// Deletes the files again, for an invoice that could not be created after all
pub fn discard(data: &Path, files: &InvoiceFiles) {
    for file in [Some(&files.pdf), files.xml.as_ref()].into_iter().flatten() {
        let _ = std::fs::remove_file(data.join(&file.path));
    }
}

/// The stored file, if it is still what it was
fn verified(data: &Path, file: &StoredFile) -> Result<PathBuf> {
    let path = data.join(&file.path);
    if !path.is_file() {
        bail!("Die gespeicherte Datei {} fehlt.", file.path);
    }
    if sha256(&path)? != file.sha256 {
        bail!(
            "Die gespeicherte Datei {} wurde verändert und wird nicht ausgegeben.",
            file.path
        );
    }
    Ok(path)
}

/// What is wrong with the kept originals: each one that is missing or no longer what its hash says
pub fn problems(data: &Path, files: &[StoredFile]) -> Vec<String> {
    files
        .iter()
        .filter_map(|file| verified(data, file).err())
        .map(|error| error.to_string())
        .collect()
}

/// Copies the originals to where the user wants them. Both are checked first, and a copy that was
/// made is removed again if another one fails.
pub fn output(
    data: &Path,
    files: &InvoiceFiles,
    pdf_target: Option<&Path>,
    xml_target: Option<&Path>,
) -> Result<()> {
    if xml_target.is_some() && files.xml.is_none() {
        bail!("Es gibt keine XML-Datei dazu.");
    }
    let wanted: Vec<(PathBuf, &Path)> = [
        (Some(&files.pdf), pdf_target),
        (files.xml.as_ref(), xml_target),
    ]
    .into_iter()
    .filter_map(|(file, target)| Some((file?, target?)))
    .map(|(file, target)| Ok((verified(data, file)?, target)))
    .collect::<Result<_>>()?;
    let mut written: Vec<&Path> = Vec::new();
    for (from, to) in wanted {
        if let Err(error) = std::fs::copy(&from, to) {
            written
                .iter()
                .for_each(|file| drop(std::fs::remove_file(file)));
            bail!("cannot save to {}: {error}", to.display());
        }
        written.push(to);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn produced(dir: &Path) -> Produced {
        std::fs::write(dir.join("a.pdf"), b"pdf bytes").unwrap();
        std::fs::write(dir.join("a.xml"), b"<xml/>").unwrap();
        Produced {
            pdf: dir.join("a.pdf"),
            xml: dir.join("a.xml"),
            report: crate::package::ValidationReport {
                valid: true,
                issues: Vec::new(),
            },
        }
    }

    fn xml_of(files: &InvoiceFiles) -> &StoredFile {
        files.xml.as_ref().expect("an invoice has an XML")
    }

    #[test]
    fn the_originals_are_kept_by_context_and_number() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();

        let files = store(data.path(), 3, "RE 2026/001", &produced(work.path()), false).unwrap();

        assert_eq!(files.pdf.path, "invoices/3/RE_2026-001.pdf");
        assert!(data.path().join(&xml_of(&files).path).is_file());
        assert_eq!(files.pdf.sha256.len(), 64);
        assert!(!files.regenerated);
    }

    #[test]
    fn they_are_handed_out_byte_for_byte() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let files = store(data.path(), 1, "RE-1", &produced(work.path()), false).unwrap();

        output(
            data.path(),
            &files,
            Some(&out.path().join("x.pdf")),
            Some(&out.path().join("x.xml")),
        )
        .unwrap();

        assert_eq!(
            std::fs::read(out.path().join("x.pdf")).unwrap(),
            b"pdf bytes"
        );
        assert_eq!(std::fs::read(out.path().join("x.xml")).unwrap(), b"<xml/>");
    }

    #[test]
    fn a_changed_or_missing_original_is_refused_and_nothing_is_written() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let files = store(data.path(), 1, "RE-1", &produced(work.path()), false).unwrap();
        std::fs::write(data.path().join(&xml_of(&files).path), b"changed").unwrap();

        let error = output(
            data.path(),
            &files,
            Some(&out.path().join("x.pdf")),
            Some(&out.path().join("x.xml")),
        )
        .unwrap_err();
        assert!(error.to_string().contains("verändert"));
        assert!(!out.path().join("x.pdf").exists());

        // the PDF alone is still fine
        output(data.path(), &files, Some(&out.path().join("x.pdf")), None).unwrap();
        std::fs::remove_file(data.path().join(&files.pdf.path)).unwrap();
        assert!(output(data.path(), &files, Some(&out.path().join("y.pdf")), None).is_err());
    }

    #[test]
    fn a_copy_that_was_made_goes_again_when_the_next_one_fails() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let files = store(data.path(), 1, "RE-1", &produced(work.path()), false).unwrap();

        let result = output(
            data.path(),
            &files,
            Some(&out.path().join("x.pdf")),
            Some(&out.path().join("missing-folder/x.xml")),
        );

        assert!(result.is_err());
        assert!(!out.path().join("x.pdf").exists());
    }

    #[test]
    fn discarding_removes_the_files() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let files = store(data.path(), 1, "RE-1", &produced(work.path()), false).unwrap();

        discard(data.path(), &files);

        assert!(!data.path().join(&files.pdf.path).exists());
        assert!(!data.path().join(&xml_of(&files).path).exists());
    }

    #[test]
    fn an_offer_keeps_its_pdf_only_in_a_folder_of_its_own() {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let pdf = produced(work.path()).pdf;

        let files = store_pdf(data.path(), Kind::Offer, 3, "AN 2026/001", &pdf).unwrap();

        assert_eq!(files.pdf.path, "offers/3/AN_2026-001.pdf");
        assert_eq!(files.xml, None);
        assert_eq!(files.pdf.sha256.len(), 64);
        assert!(!data.path().join("invoices").exists());

        // handed out as it was kept, and there is no XML to hand out
        output(data.path(), &files, Some(&out.path().join("a.pdf")), None).unwrap();
        assert_eq!(
            std::fs::read(out.path().join("a.pdf")).unwrap(),
            b"pdf bytes"
        );
        assert!(output(
            data.path(),
            &files,
            Some(&out.path().join("b.pdf")),
            Some(&out.path().join("b.xml"))
        )
        .is_err());
        assert!(!out.path().join("b.pdf").exists());

        discard(data.path(), &files);
        assert!(!data.path().join(&files.pdf.path).exists());
    }
}
