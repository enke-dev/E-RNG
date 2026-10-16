use std::path::PathBuf;

use tauri::State;

use crate::session::Sessions;

/// Deletes a work directory made by `produce_invoice`, e.g. after saving or cancelling.
#[tauri::command]
pub fn discard_work_dir(sessions: State<'_, Sessions>, work_dir: PathBuf) {
    sessions.discard(&work_dir);
}
