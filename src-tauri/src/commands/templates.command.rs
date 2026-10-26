use tauri::State;

use crate::model::PositionTemplate;
use crate::store::Store;
use crate::utils::{CommandError, CommandResult};

#[tauri::command]
pub fn templates_list(
    store: State<'_, Store>,
    context_id: i64,
) -> CommandResult<Vec<PositionTemplate>> {
    store.templates(context_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn template_save(
    store: State<'_, Store>,
    template: PositionTemplate,
) -> CommandResult<PositionTemplate> {
    store.save_template(&template).map_err(CommandError::from)
}

#[tauri::command]
pub fn template_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.delete_template(id).map_err(CommandError::from)
}
