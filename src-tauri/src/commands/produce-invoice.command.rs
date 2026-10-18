use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::draft::{build_invoice, totals};
use crate::model::Status;
use crate::package::ValidationReport;
use crate::pipeline::{self, ProduceError};
use crate::render::Branding;
use crate::session::{Session, Sessions};
use crate::sidecar::Sidecars;
use crate::store::Store;

#[derive(Debug, Serialize)]
pub struct ProduceResult {
    /// Handle for `export_invoice` and `discard_work_dir`
    pub work_dir: PathBuf,
    /// The number the files carry, final once they are exported
    pub number: String,
    pub report: ValidationReport,
}

fn failed(error: impl std::fmt::Display) -> ProduceError {
    ProduceError::Failed {
        message: crate::utils::message(error),
    }
}

/// Generates the XML and the PDF of a stored invoice in a fresh work directory and validates them.
/// The files stay there until they are exported (only when valid) or the directory is discarded.
#[tauri::command]
pub async fn produce_invoice(
    app: AppHandle,
    store: State<'_, Store>,
    sessions: State<'_, Sessions>,
    id: i64,
) -> Result<ProduceResult, ProduceError> {
    let sidecars = Sidecars::locate(&app).map_err(failed)?;
    let stored = store.invoice(id).map_err(failed)?;
    let current = store.settings().map_err(failed)?;
    let settings = stored.draft.frozen.clone().unwrap_or(current);

    // exported invoices keep their number, drafts get the next free one (taken on export only)
    let number = match &stored.number {
        Some(number) => number.clone(),
        None => {
            let year = stored
                .draft
                .issue_date
                .get(..4)
                .and_then(|y| y.parse().ok())
                .unwrap_or(1970);
            store
                .next_number(&settings.number_prefix, year)
                .map_err(failed)?
        }
    };
    let invoice = build_invoice(&stored.draft, &settings, &number)
        .map_err(|problems| ProduceError::Input { problems })?;
    let total = totals(&invoice).gross;
    let customs = store.custom_layouts().map_err(failed)?;
    let branding = Branding::for_draft(&stored.draft, &settings, &customs);
    let profile = stored.draft.profile;
    let produced_layout = branding.custom.clone();

    let work_dir = tempfile::Builder::new()
        .prefix("erng-")
        .tempdir()
        .map_err(failed)?
        .keep();
    let dir = work_dir.clone();
    let produced = tauri::async_runtime::spawn_blocking(move || {
        pipeline::produce(&sidecars, &dir, &invoice, profile, &branding)
    })
    .await
    .map_err(failed)?;

    match produced {
        Ok(produced) => {
            sessions.insert(
                work_dir.clone(),
                Session {
                    valid: produced.report.valid,
                    pdf: produced.pdf,
                    xml: produced.xml,
                    invoice_id: id,
                    number: number.clone(),
                    settings,
                    custom_layout: produced_layout,
                    total,
                },
            );
            debug_assert!(
                stored.status == Status::Draft || stored.number.as_deref() == Some(&number)
            );
            Ok(ProduceResult {
                work_dir,
                number,
                report: produced.report,
            })
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&work_dir);
            Err(error)
        }
    }
}
