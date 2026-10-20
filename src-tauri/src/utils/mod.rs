// shared helpers for the commands

use std::fmt::Display;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Where the app keeps its database and files. A debug build (`tauri dev`) gets a folder of its
/// own: an installed older version refuses a database that a newer one has migrated, so trying
/// things must never touch the data of the installed app.
pub fn data_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    if !cfg!(debug_assertions) {
        return Ok(dir);
    }
    let name = dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    Ok(dir.with_file_name(format!("{name}.dev")))
}

/// The date the UI starts new and duplicated invoices with.
/// The day an invoice is issued, for numbering; a date that is none is reported elsewhere.
pub fn issue_day(issue_date: &str) -> en16931::Date {
    en16931::Date::parse(issue_date).unwrap_or_else(|_| en16931::Date::new(1970, 1, 1).unwrap())
}

pub fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// Errors cross the IPC boundary as readable text.
pub fn message(error: impl Display) -> String {
    format!("{error:#}")
}
