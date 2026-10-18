use std::path::PathBuf;

use tauri::State;

use crate::model::{Status, StoredInvoice};
use crate::session::{Artifact, Sessions};
use crate::store::Store;
use crate::utils::message;

/// Saves the produced files where the user picked, but only if the invoice passed validation.
/// A draft becomes final with this: it keeps the number printed on the files and is locked.
#[tauri::command]
pub fn export_invoice(
    store: State<'_, Store>,
    sessions: State<'_, Sessions>,
    id: i64,
    work_dir: PathBuf,
    pdf_target: Option<PathBuf>,
    xml_target: Option<PathBuf>,
) -> Result<StoredInvoice, String> {
    let session = sessions.exportable(&work_dir, id)?;

    let mut written = Vec::new();
    for (artifact, target) in [(Artifact::Pdf, pdf_target), (Artifact::Xml, xml_target)] {
        let Some(target) = target else { continue };
        if let Err(error) = std::fs::copy(Sessions::file(&session, artifact), &target) {
            written
                .iter()
                .for_each(|file| drop(std::fs::remove_file(file)));
            return Err(format!("cannot save to {}: {error}", target.display()));
        }
        written.push(target);
    }

    let stored = store.invoice(id).map_err(message)?;
    let result = match stored.status {
        Status::Exported => Ok(stored),
        Status::Draft => store.finalize(
            id,
            &session.number,
            &session.settings,
            session.custom_layout.as_ref(),
            &session.total,
        ),
    };
    match result {
        Ok(invoice) => {
            sessions.discard(&work_dir);
            Ok(invoice)
        }
        Err(error) => {
            // the files carry a number the invoice did not get, do not leave them behind
            written
                .iter()
                .for_each(|file| drop(std::fs::remove_file(file)));
            Err(message(error))
        }
    }
}
