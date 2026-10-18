mod commands;
mod draft;
mod invoice;
mod model;
mod package;
mod pipeline;
mod render;
mod session;
mod sidecar;
mod store;
mod utils;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(store::Store::open(&dir.join("e-rng.sqlite3"))?);
            Ok(())
        })
        .manage(session::Sessions::default())
        .invoke_handler(tauri::generate_handler![
            commands::check_invoice_command::check_invoice,
            commands::customers_command::customer_delete,
            commands::customers_command::customer_save,
            commands::customers_command::customers_list,
            commands::discard_work_dir_command::discard_work_dir,
            commands::export_invoice_command::export_invoice,
            commands::invoices_command::invoice_blank,
            commands::invoices_command::invoice_delete,
            commands::invoices_command::invoice_duplicate,
            commands::invoices_command::invoice_get,
            commands::invoices_command::invoice_save,
            commands::invoices_command::invoices_list,
            commands::produce_invoice_command::produce_invoice,
            commands::settings_command::settings_get,
            commands::settings_command::settings_save,
            commands::templates_command::template_delete,
            commands::templates_command::template_save,
            commands::templates_command::templates_list,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // work directories hold unsaved invoices, none of them outlives the app
            if let tauri::RunEvent::Exit = event {
                app.state::<session::Sessions>().discard_all();
            }
        });
}
