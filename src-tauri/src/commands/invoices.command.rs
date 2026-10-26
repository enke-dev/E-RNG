use tauri::State;

use crate::draft::{build_invoice, totals};
use crate::model::{InvoiceDraft, ListEntry, StoredInvoice};
use crate::store::Store;
use crate::utils::{today, CommandError, CommandResult};

#[tauri::command]
pub fn invoices_list(
    store: State<'_, Store>,
    context_id: Option<i64>,
) -> CommandResult<Vec<ListEntry>> {
    store.invoices(context_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn invoice_get(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store.invoice(id).map_err(CommandError::from)
}

/// A blank draft dated today, not stored until it is saved.
#[tauri::command]
pub fn invoice_blank() -> InvoiceDraft {
    InvoiceDraft::blank(&today())
}

/// Marks an exported invoice as paid on a date (ISO), or as open again.
#[tauri::command]
pub fn invoice_set_paid(
    store: State<'_, Store>,
    id: i64,
    paid_at: Option<String>,
) -> CommandResult<StoredInvoice> {
    store
        .set_paid(id, paid_at.as_deref())
        .map_err(CommandError::from)
}

/// Creates (`id` is `None`, under `context_id`) or updates a draft, which stays under the context it
/// was made under. Whatever state the form is in is saved, the total shown in the list is kept only
/// while the draft can be calculated.
#[tauri::command]
pub fn invoice_save(
    store: State<'_, Store>,
    id: Option<i64>,
    context_id: Option<i64>,
    draft: InvoiceDraft,
) -> CommandResult<StoredInvoice> {
    let context_id = match (id, context_id) {
        (Some(id), _) => store.invoice(id)?.context_id,
        (None, Some(context_id)) => context_id,
        (None, None) => return Err("Für eine neue Rechnung fehlt der Kontext.".into()),
    };
    let settings = store.settings(context_id)?;
    let total = build_invoice(&draft, &settings, "ENTWURF")
        .ok()
        .map(|invoice| totals(&invoice).gross);
    store
        .save_draft(id, context_id, &draft, total.as_deref())
        .map_err(CommandError::from)
}

/// Takes an exported invoice out of the list, it stays as it is.
#[tauri::command]
pub fn invoice_archive(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.archive_invoice(id).map_err(CommandError::from)
}

#[tauri::command]
pub fn invoice_duplicate(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store.duplicate(id, &today()).map_err(CommandError::from)
}

#[tauri::command]
pub fn invoice_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.delete_draft(id).map_err(CommandError::from)
}
