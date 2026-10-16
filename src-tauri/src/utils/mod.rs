// shared helpers for the commands

use std::fmt::Display;

/// The date the UI starts new and duplicated invoices with.
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
