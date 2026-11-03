use tauri::State;

use crate::case_graph::CaseGraph;
use crate::model::{Case, Event, StoredInvoice};
use crate::store::{Sequence, Store};
use crate::utils::{today, CommandError, CommandResult};

/// The Vorgänge of a context (all with none), newest first
#[tauri::command]
pub fn cases_list(
    store: State<'_, Store>,
    context_id: Option<i64>,
    with_archived: bool,
) -> CommandResult<Vec<Case>> {
    store
        .cases(context_id, with_archived)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn case_get(store: State<'_, Store>, id: i64) -> CommandResult<Case> {
    store.case(id).map_err(CommandError::from)
}

/// Starts a Vorgang for a customer, without documents yet
#[tauri::command]
pub fn case_create(
    store: State<'_, Store>,
    context_id: i64,
    customer_id: Option<i64>,
    title: Option<String>,
) -> CommandResult<Case> {
    store
        .create_case(context_id, customer_id, title.as_deref())
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn case_set_title(
    store: State<'_, Store>,
    id: i64,
    title: Option<String>,
) -> CommandResult<Case> {
    store
        .set_case_title(id, title.as_deref())
        .map_err(CommandError::from)
}

/// Removes a Vorgang without documents
#[tauri::command]
pub fn case_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.delete_case(id).map_err(CommandError::from)
}

/// The documents of a Vorgang and how they relate, for its graph
#[tauri::command]
pub fn case_graph(store: State<'_, Store>, id: i64) -> CommandResult<CaseGraph> {
    store.case_graph(id).map_err(CommandError::from)
}

/// The history of all documents of a Vorgang, as recorded
#[tauri::command]
pub fn case_events(store: State<'_, Store>, id: i64) -> CommandResult<Vec<Event>> {
    store.case_events(id).map_err(CommandError::from)
}

/// Where the numbers of a context's Vorgänge stand today, by its saved settings
#[tauri::command]
pub fn case_sequence(store: State<'_, Store>, context_id: i64) -> CommandResult<Sequence> {
    let settings = store.settings(context_id)?;
    store
        .case_sequence(context_id, &settings)
        .map_err(CommandError::from)
}

/// Continues the numbers of the Vorgänge of today's period from `next`, saved at once
#[tauri::command]
pub fn case_sequence_set(
    store: State<'_, Store>,
    context_id: i64,
    next: u32,
) -> CommandResult<Sequence> {
    let settings = store.settings(context_id)?;
    store
        .set_case_counter(context_id, &settings, next)
        .map_err(CommandError::from)
}

/// A Folgeangebot: the created offer `id` copied into a new Vorgang that follows its own
#[tauri::command]
pub fn offer_follow(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store.follow_offer(id, &today()).map_err(CommandError::from)
}

/// A Sammelrechnung over Vorgänge of one customer: a new Vorgang collecting them, with an invoice
/// draft of all that is open of their offers
#[tauri::command]
pub fn cases_collect(store: State<'_, Store>, ids: Vec<i64>) -> CommandResult<StoredInvoice> {
    store.collect(&ids, &today()).map_err(CommandError::from)
}
