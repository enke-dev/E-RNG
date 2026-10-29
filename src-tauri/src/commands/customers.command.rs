use tauri::State;

use crate::model::Customer;
use crate::store::Store;
use crate::utils::{CommandError, CommandResult};

#[tauri::command]
pub fn customers_list(store: State<'_, Store>, context_id: i64) -> CommandResult<Vec<Customer>> {
    store.customers(context_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn customer_save(store: State<'_, Store>, customer: Customer) -> CommandResult<Customer> {
    // what a paste brings along ends up in the XML and the PDF, so it goes before saving
    store
        .save_customer(&customer.cleaned())
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn customer_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.delete_customer(id).map_err(CommandError::from)
}
