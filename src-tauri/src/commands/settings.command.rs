use tauri::State;

use crate::model::Settings;
use crate::store::Store;
use crate::utils::message;

#[tauri::command]
pub fn settings_get(store: State<'_, Store>) -> Result<Settings, String> {
    store.settings().map_err(message)
}

#[tauri::command]
pub fn settings_save(store: State<'_, Store>, settings: Settings) -> Result<(), String> {
    let archived = store
        .custom_layouts()
        .map_err(message)?
        .iter()
        .any(|l| l.archived && l.key() == settings.layout);
    if archived {
        return Err("Ein archiviertes Layout kann nicht das Standard-Layout sein.".into());
    }
    store.save_settings(&settings).map_err(message)
}
