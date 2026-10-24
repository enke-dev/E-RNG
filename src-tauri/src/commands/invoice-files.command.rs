use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::creation::{self, Created, Creating};
use crate::model::StoredInvoice;
use crate::pipeline::ProduceError;
use crate::sidecar::Sidecars;
use crate::store::Store;
use crate::utils::data_dir;

fn failed(error: impl std::fmt::Display) -> ProduceError {
    ProduceError::Failed {
        message: crate::utils::message(error),
    }
}

/// The number the draft would get if it were created now, for asking before it is.
#[tauri::command]
pub fn invoice_next_number(store: State<'_, Store>, id: i64) -> Result<String, ProduceError> {
    creation::next_number(&store, id)
}

/// Turns a draft into an invoice: checks, numbers, freezes and locks it and keeps its PDF and XML.
/// If anything fails nothing is stored and the draft stays as it was.
#[tauri::command]
pub async fn invoice_create(app: AppHandle, id: i64) -> Result<Created, ProduceError> {
    let sidecars = Sidecars::locate(&app).map_err(failed)?;
    let data = data_dir(&app).map_err(failed)?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        let creating = app.state::<Creating>();
        creation::create(&sidecars, &store, &creating, &data, id)
    })
    .await
    .map_err(failed)?
}

/// Copies the kept originals of an invoice to the places the user picked. An invoice from before
/// the originals were kept gets them made again first.
#[tauri::command]
pub async fn invoice_output(
    app: AppHandle,
    id: i64,
    pdf_target: Option<PathBuf>,
    xml_target: Option<PathBuf>,
) -> Result<StoredInvoice, ProduceError> {
    let sidecars = Sidecars::locate(&app).map_err(failed)?;
    let data = data_dir(&app).map_err(failed)?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        creation::output(
            &sidecars,
            &store,
            &data,
            id,
            pdf_target.as_deref(),
            xml_target.as_deref(),
        )
    })
    .await
    .map_err(failed)?
}
