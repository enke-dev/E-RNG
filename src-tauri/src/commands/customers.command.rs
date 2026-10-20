use tauri::State;

use crate::model::Customer;
use crate::store::Store;
use crate::utils::message;

#[tauri::command]
pub fn customers_list(store: State<'_, Store>, context_id: i64) -> Result<Vec<Customer>, String> {
    store.customers(context_id).map_err(message)
}

#[tauri::command]
pub fn customer_save(store: State<'_, Store>, customer: Customer) -> Result<Customer, String> {
    store.save_customer(&customer).map_err(message)
}

#[tauri::command]
pub fn customer_delete(store: State<'_, Store>, id: i64) -> Result<(), String> {
    store.delete_customer(id).map_err(message)
}
