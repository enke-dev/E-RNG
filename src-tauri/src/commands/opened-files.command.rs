//! Layout and backup files the system hands to the app (double click in Finder, "Open With"). They
//! can arrive before the window is ready, so they wait here until the frontend asks for them.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};

/// The endings of layout and backup files, the same the file associations and the dialogs use
const EXTENSIONS: [&str; 2] = ["erng-layout", crate::backup::EXTENSION];

#[derive(Default)]
pub struct OpenedFiles(Mutex<Vec<PathBuf>>);

/// The layout and backup files among what the system opened, others are none of our business.
fn our_files(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|e| EXTENSIONS.iter().any(|ours| e.eq_ignore_ascii_case(ours)))
        })
        .collect()
}

/// The files of a start from the command line, as Windows and Linux open a file with the app: the
/// program comes first, relative paths are taken in the folder it was started in. Flags and other
/// files are dropped by their ending.
// macOS hands files over with an event (see `lib.rs`)
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn from_args(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter().skip(1).map(|arg| cwd.join(arg)).collect()
}

/// Keeps the files and tells the window there is something to ask for.
pub fn offer(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let files = our_files(paths);
    if files.is_empty() {
        return;
    }
    if let Ok(mut waiting) = app.state::<OpenedFiles>().0.lock() {
        waiting.extend(files);
    }
    let _ = app.emit("file-opened", ());
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
    fn only_layout_and_backup_files_are_taken() {
        let dir = std::env::temp_dir();
        let paths = [
            "a.erng-layout",
            "B.ERNG-LAYOUT",
            "c.pdf",
            "e.erng-backup",
            "f.zip",
        ]
        .map(|name| dir.join(name));

        assert_eq!(
            our_files(paths),
            [
                dir.join("a.erng-layout"),
                dir.join("B.ERNG-LAYOUT"),
                dir.join("e.erng-backup")
            ]
        );
    }

    #[test]
    fn arguments_are_taken_in_the_folder_the_app_started_in() {
        let cwd = std::env::temp_dir();
        let elsewhere = cwd.join("elsewhere").join("b.erng-layout");
        let args = [
            OsString::from("e-rng"),
            OsString::from("a.erng-layout"),
            OsString::from("--flag"),
            elsewhere.clone().into_os_string(),
        ];

        assert_eq!(
            our_files(from_args(args, &cwd)),
            [cwd.join("a.erng-layout"), elsewhere]
        );
    }
}
