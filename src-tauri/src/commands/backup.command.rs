use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::backup;
use crate::creation::Creating;
use crate::store::{DataCounts, Store};
use crate::utils::{data_dir, CommandError, CommandResult};

/// The word the user types to confirm removing everything
const REMOVE_WORD: &str = "ENTFERNEN";

/// Where the app stands with its backups
#[derive(Debug, Serialize)]
pub struct BackupStatus {
    /// UTC, none if there never was a backup
    pub last_backup_at: Option<String>,
    /// The folder the automatic safety backups go to
    pub safety_dir: String,
}

#[derive(Debug, Serialize)]
pub struct BackupCreated {
    pub path: String,
    pub files: usize,
    pub bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct RestoreOutcome {
    pub counts: DataCounts,
    pub safety_copy: String,
    /// Kept originals that are missing or changed in the restored data
    pub problems: Vec<String>,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i32,
}

#[derive(Debug, Serialize)]
pub struct RemoveOutcome {
    pub safety_copy: String,
}

fn busy(app: &AppHandle) -> CommandResult<()> {
    if app.state::<Creating>().any() {
        return Err("Gerade wird eine Rechnung erstellt. Bitte warten, bis sie fertig ist.".into());
    }
    Ok(())
}

fn version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn text(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

/// When the last backup was made, and where the safety backups go
#[tauri::command]
pub fn backup_status(app: AppHandle, store: State<'_, Store>) -> CommandResult<BackupStatus> {
    Ok(BackupStatus {
        last_backup_at: store.last_backup_at()?,
        safety_dir: text(&backup::safety_dir(&data_dir(&app)?)),
    })
}

/// Writes a backup of all data to the file the user picked
#[tauri::command]
pub async fn backup_create(app: AppHandle, path: PathBuf) -> CommandResult<BackupCreated> {
    let data = data_dir(&app)?;
    let app_version = version(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        backup::create(&store, &data, &path, &app_version, true)
    })
    .await
    .map_err(CommandError::from)?
    .map(|created| BackupCreated {
        path: text(&created.path),
        files: created.files,
        bytes: created.bytes,
        created_at: created.created_at,
    })
    .map_err(CommandError::from)
}

/// Replaces all data by a backup, after checking it and after a safety backup of the data in use.
/// Nothing changes when the file is refused or anything fails.
#[tauri::command]
pub async fn backup_restore(app: AppHandle, path: PathBuf) -> CommandResult<RestoreOutcome> {
    busy(&app)?;
    let data = data_dir(&app)?;
    let app_version = version(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        backup::restore(&store, &data, &path, &app_version)
    })
    .await
    .map_err(CommandError::from)?
    .map(|restored| RestoreOutcome {
        counts: restored.counts,
        safety_copy: text(&restored.safety_copy),
        problems: restored.problems,
        created_at: restored.created_at,
        app_version: restored.app_version,
        schema_version: restored.schema_version,
    })
    .map_err(CommandError::from)
}

/// Resets the app to a first start. A safety backup is made first; the word must be typed as the
/// dialog asks, the check is here as well so no other caller can skip it.
#[tauri::command]
pub async fn data_remove(app: AppHandle, confirmation: String) -> CommandResult<RemoveOutcome> {
    if confirmation.trim() != REMOVE_WORD {
        return Err(format!("Zur Bestätigung „{REMOVE_WORD}“ eingeben.").into());
    }
    busy(&app)?;
    let data = data_dir(&app)?;
    let app_version = version(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        backup::remove_all(&store, &data, &app_version)
    })
    .await
    .map_err(CommandError::from)?
    .map(|removed| RemoveOutcome {
        safety_copy: text(&removed.safety_copy),
    })
    .map_err(CommandError::from)
}
