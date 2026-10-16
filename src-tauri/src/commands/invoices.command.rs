use tauri::State;

use crate::draft::{build_invoice, totals};
use crate::model::{InvoiceDraft, ListEntry, StoredInvoice};
use crate::store::Store;
use crate::utils::{message, today};

#[tauri::command]
pub fn invoices_list(store: State<'_, Store>) -> Result<Vec<ListEntry>, String> {
    store.invoices().map_err(message)
}

#[tauri::command]
pub fn invoice_get(store: State<'_, Store>, id: i64) -> Result<StoredInvoice, String> {
    store.invoice(id).map_err(message)
}

/// A blank draft dated today, not stored until it is saved.
#[tauri::command]
pub fn invoice_blank() -> InvoiceDraft {
    InvoiceDraft::blank(&today())
}

/// Creates (`id` is `None`) or updates a draft. Whatever state the form is in is saved, the total
/// shown in the list is kept only while the draft can be calculated.
#[tauri::command]
pub fn invoice_save(
    store: State<'_, Store>,
    id: Option<i64>,
    draft: InvoiceDraft,
) -> Result<StoredInvoice, String> {
    let settings = store.settings().map_err(message)?;
    let total = build_invoice(&draft, &settings, "ENTWURF")
        .ok()
        .map(|invoice| totals(&invoice).gross);
    store
        .save_draft(id, &draft, total.as_deref())
        .map_err(message)
}

#[tauri::command]
pub fn invoice_duplicate(store: State<'_, Store>, id: i64) -> Result<StoredInvoice, String> {
    store.duplicate(id, &today()).map_err(message)
}

#[tauri::command]
pub fn invoice_delete(store: State<'_, Store>, id: i64) -> Result<(), String> {
    store.delete_draft(id).map_err(message)
}
