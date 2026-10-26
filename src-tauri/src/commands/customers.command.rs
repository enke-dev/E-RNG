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
    store.save_customer(&customer).map_err(CommandError::from)
}

#[tauri::command]
pub fn customer_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.delete_customer(id).map_err(CommandError::from)
}
