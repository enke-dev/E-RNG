//! Layout files the system hands to the app (double click in Finder, "Open With"). They can arrive
//! before the window is ready, so they wait here until the frontend asks for them.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, Url};

/// The ending of a layout file, the same the file association and the dialogs use
// macOS hands files over with an event, elsewhere nothing here is used (see `lib.rs`)
#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
const EXTENSION: &str = "erng-layout";

#[derive(Default)]
pub struct OpenedFiles(Mutex<Vec<PathBuf>>);

/// The layout files among what the system opened, others are none of our business.
#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
fn layout_files(urls: &[Url]) -> Vec<PathBuf> {
    urls.iter()
        .filter_map(|url| url.to_file_path().ok())
        .filter(|path| {
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
        })
        .collect()
}

/// Keeps the files and tells the window there is something to ask for.
#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
pub fn offer(app: &AppHandle, urls: &[Url]) {
    let files = layout_files(urls);
    if files.is_empty() {
        return;
    }
    if let Ok(mut waiting) = app.state::<OpenedFiles>().0.lock() {
        waiting.extend(files);
    }
    let _ = app.emit("layout-file-opened", ());
}

/// What was opened since the last call.
#[tauri::command]
pub fn opened_files(files: State<'_, OpenedFiles>) -> Vec<PathBuf> {
    files
        .0
        .lock()
        .map(|mut waiting| std::mem::take(&mut *waiting))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_layout_files_are_taken() {
        let urls = [
            "file:///tmp/a.erng-layout",
            "file:///tmp/B.ERNG-LAYOUT",
            "file:///tmp/c.pdf",
            "https://x.test/d.erng-layout",
        ]
        .map(|u| Url::parse(u).unwrap());

        assert_eq!(
            layout_files(&urls),
            [
                PathBuf::from("/tmp/a.erng-layout"),
                PathBuf::from("/tmp/B.ERNG-LAYOUT")
            ]
        );
    }
}
