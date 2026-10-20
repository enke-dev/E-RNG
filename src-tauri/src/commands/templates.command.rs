use tauri::State;

use crate::model::PositionTemplate;
use crate::store::Store;
use crate::utils::message;

#[tauri::command]
pub fn templates_list(
    store: State<'_, Store>,
    context_id: i64,
) -> Result<Vec<PositionTemplate>, String> {
    store.templates(context_id).map_err(message)
}

#[tauri::command]
pub fn template_save(
    store: State<'_, Store>,
    template: PositionTemplate,
) -> Result<PositionTemplate, String> {
    store.save_template(&template).map_err(message)
}

#[tauri::command]
pub fn template_delete(store: State<'_, Store>, id: i64) -> Result<(), String> {
    store.delete_template(id).map_err(message)
}
