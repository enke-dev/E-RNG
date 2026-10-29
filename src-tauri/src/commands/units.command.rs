use std::collections::BTreeMap;

use tauri::State;

use crate::store::Store;
use crate::units::{catalog, CatalogEntry, Unit, UnitUsage};
use crate::utils::{CommandError, CommandResult};

/// The units of a context in the order the user keeps them
#[tauri::command]
pub fn units_list(store: State<'_, Store>, context_id: i64) -> CommandResult<Vec<Unit>> {
    store.units(context_id).map_err(CommandError::from)
}

/// Every code with its names, the common ones first
#[tauri::command]
pub fn units_catalog() -> &'static [CatalogEntry] {
    catalog()
}

/// How many drafts and saved positions use each unit of a context, by unit id
#[tauri::command]
pub fn units_counts(
    store: State<'_, Store>,
    context_id: i64,
) -> CommandResult<BTreeMap<i64, usize>> {
    store.unit_counts(context_id).map_err(CommandError::from)
}

/// Adds a unit (`id` is `None`) or changes one; drafts and saved positions follow a change
#[tauri::command]
pub fn unit_save(store: State<'_, Store>, unit: Unit) -> CommandResult<Unit> {
    store.save_unit(&unit).map_err(CommandError::from)
}

/// What a change or the deletion of a unit reaches
#[tauri::command]
pub fn unit_usage(store: State<'_, Store>, id: i64) -> CommandResult<UnitUsage> {
    store.unit_usage(id).map_err(CommandError::from)
}

/// Deletes a unit, a used one needs the unit that takes its place
#[tauri::command]
pub fn unit_delete(
    store: State<'_, Store>,
    id: i64,
    replacement: Option<i64>,
) -> CommandResult<()> {
    store
        .delete_unit(id, replacement)
        .map_err(CommandError::from)
}

/// Puts the units of a context in the order of `ids`
#[tauri::command]
pub fn units_reorder(store: State<'_, Store>, context_id: i64, ids: Vec<i64>) -> CommandResult<()> {
    store
        .reorder_units(context_id, &ids)
        .map_err(CommandError::from)
}

/// Makes the unit called `label` the one new positions of a context start with, none: the first
#[tauri::command]
pub fn unit_default(
    store: State<'_, Store>,
    context_id: i64,
    label: Option<String>,
) -> CommandResult<()> {
    store
        .choose_default_unit(context_id, label.as_deref())
        .map_err(CommandError::from)
}
