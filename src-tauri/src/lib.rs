mod commands;
mod context;
mod creation;
mod draft;
mod invoice;
mod layout;
mod layout_bundle;
mod layout_files;
mod model;
mod numbering;
mod originals;
mod package;
mod pipeline;
mod render;
mod sidecar;
mod store;
mod utils;

use tauri::menu::{Menu, MenuItem, MenuItemKind};
use tauri::{Emitter, Manager};

const UPDATE_CHECK_MENU_ID: &str = "check-for-updates";
const SETTINGS_MENU_ID: &str = "open-settings";

/// Puts "look for updates" and "settings" into the menu bar: under "About" in the app menu on macOS
/// (settings with the usual shortcut), at the end of the last menu elsewhere.
fn add_entries_to_menu(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();
    let menu = if let Some(menu) = app.menu() {
        menu
    } else {
        let menu = Menu::default(handle)?;
        app.set_menu(menu.clone())?;
        menu
    };
    let item = MenuItem::with_id(
        handle,
        UPDATE_CHECK_MENU_ID,
        "Nach Updates suchen",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(
        handle,
        SETTINGS_MENU_ID,
        "Einstellungen",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let submenus: Vec<_> = menu
        .items()?
        .into_iter()
        .filter_map(|entry| match entry {
            MenuItemKind::Submenu(submenu) => Some(submenu),
            _ => None,
        })
        .collect();
    if cfg!(target_os = "macos") {
        // the app menu is the first one, "About" its first entry
        if let Some(app_menu) = submenus.first() {
            app_menu.insert(&item, 1)?;
            app_menu.insert(&settings, 2)?;
        }
    } else if let Some(last) = submenus.last() {
        last.append(&item)?;
        last.append(&settings)?;
    }
    Ok(())
}

/// Starts the app and blocks until the last window is closed.
///
/// # Panics
///
/// When the window or the plugins cannot be set up: the app is of no use then.
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
            add_entries_to_menu(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            // the window does what the entry says: it looks for an update and says what it found, or
            // opens the settings
            if event.id() == UPDATE_CHECK_MENU_ID {
                let _ = app.emit("check-for-updates", ());
            } else if event.id() == SETTINGS_MENU_ID {
                let _ = app.emit("open-settings", ());
            }
        })
        .manage(creation::Creating::default())
        .manage(commands::opened_files_command::OpenedFiles::default())
        .invoke_handler(tauri::generate_handler![
            commands::check_invoice_command::check_invoice,
            commands::contexts_command::context_archive,
            commands::contexts_command::context_blank,
            commands::contexts_command::context_brands,
            commands::contexts_command::context_remove,
            commands::contexts_command::context_save,
            commands::contexts_command::context_usage,
            commands::contexts_command::contexts_list,
            commands::contexts_command::number_example,
            commands::contexts_command::settings_check,
            commands::customers_command::customer_delete,
            commands::customers_command::customer_save,
            commands::customers_command::customers_list,
            commands::invoice_files_command::invoice_create,
            commands::invoice_files_command::invoice_next_number,
            commands::invoice_files_command::invoice_output,
            commands::invoices_command::invoice_archive,
            commands::invoices_command::invoice_unarchive,
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
            commands::templates_command::template_delete,
            commands::templates_command::template_save,
            commands::templates_command::templates_list,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // a layout file was opened with the app (macOS says so with an event)
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = event {
                commands::opened_files_command::offer(app, &urls);
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            let _ = (app, event);
        });
}
