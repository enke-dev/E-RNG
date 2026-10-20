mod commands;
mod draft;
mod invoice;
mod layout;
mod layout_bundle;
mod layout_files;
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
        // size, place and state of the window come back at the next start
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = utils::data_dir(app.handle())?;
            std::fs::create_dir_all(&dir)?;
            let store = store::Store::open(&dir.join("e-rng.sqlite3"))?;
            // files of deleted or replaced stationery and fonts, nothing is being edited yet
            if let Ok(layouts) = store.layouts_in_use() {
                let _ = layout_files::prune(&dir.join("layouts"), &layouts);
            }
            app.manage(store);
            Ok(())
        })
        .manage(session::Sessions::default())
        .manage(commands::opened_files_command::OpenedFiles::default())
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
            commands::invoices_command::invoice_set_paid,
            commands::invoices_command::invoices_list,
            commands::layouts_command::layout_delete,
            commands::layouts_command::layout_builtin,
            commands::layouts_command::layout_export,
            commands::layouts_command::layout_import,
            commands::layouts_command::layout_import_font,
            commands::layouts_command::layout_import_image,
            commands::layouts_command::layout_preview,
            commands::layouts_command::layout_save,
            commands::layouts_command::layout_usage,
            commands::layouts_command::layouts_custom,
            commands::layouts_command::layouts_list,
            commands::opened_files_command::opened_files,
            commands::preview_invoice_command::preview_invoice,
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
            match event {
                tauri::RunEvent::Exit => app.state::<session::Sessions>().discard_all(),
                // a layout file was opened with the app (macOS says so with an event)
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                tauri::RunEvent::Opened { urls } => {
                    commands::opened_files_command::offer(app, &urls);
                }
                _ => {}
            }
        });
}
