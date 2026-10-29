use std::collections::BTreeMap;

use tauri::State;

use crate::rates::{Rate, RateUsage};
use crate::store::Store;
use crate::utils::{CommandError, CommandResult};

/// The VAT rates of a context in the order the user keeps them
#[tauri::command]
pub fn rates_list(store: State<'_, Store>, context_id: i64) -> CommandResult<Vec<Rate>> {
    store.rates(context_id).map_err(CommandError::from)
}

/// How many drafts and saved positions use each rate of a context, by rate id
#[tauri::command]
pub fn rates_counts(
    store: State<'_, Store>,
    context_id: i64,
) -> CommandResult<BTreeMap<i64, usize>> {
    store.rate_counts(context_id).map_err(CommandError::from)
}

/// Adds a rate (`id` is `None`) or changes one; drafts, saved positions and the default of the
/// context follow a change
#[tauri::command]
pub fn rate_save(store: State<'_, Store>, rate: Rate) -> CommandResult<Rate> {
    store.save_rate(&rate).map_err(CommandError::from)
}

/// What a change or the deletion of a rate reaches
#[tauri::command]
pub fn rate_usage(store: State<'_, Store>, id: i64) -> CommandResult<RateUsage> {
    store.rate_usage(id).map_err(CommandError::from)
}

/// Deletes a rate, a used one needs the rate that takes its place
#[tauri::command]
pub fn rate_delete(
    store: State<'_, Store>,
    id: i64,
    replacement: Option<i64>,
) -> CommandResult<()> {
    store
        .delete_rate(id, replacement)
        .map_err(CommandError::from)
}

/// Puts the rates of a context in the order of `ids`
#[tauri::command]
pub fn rates_reorder(store: State<'_, Store>, context_id: i64, ids: Vec<i64>) -> CommandResult<()> {
    store
        .reorder_rates(context_id, &ids)
        .map_err(CommandError::from)
}

/// Makes a rate of the list the default of a context, blank: the context charges no VAT
#[tauri::command]
pub fn rate_default(store: State<'_, Store>, context_id: i64, rate: String) -> CommandResult<()> {
    store
        .choose_default_rate(context_id, &rate)
        .map_err(CommandError::from)
}
