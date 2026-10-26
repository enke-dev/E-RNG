use tauri::State;

use crate::context::{Context, ContextUsage, Disposition, Missing, BRANDS};
use crate::model::Settings;
use crate::numbering::Pattern;
use crate::store::Store;
use crate::utils::{CommandError, CommandResult};

/// Every context with what is missing in it, archived ones included (the UI hides them)
#[tauri::command]
pub fn contexts_list(store: State<'_, Store>) -> CommandResult<Vec<Context>> {
    store.contexts().map_err(CommandError::from)
}

/// Creates (`id` is `None`) or updates a context. An incomplete one is saved as well, the data is
/// filled in step by step, `missing` tells what is still to do.
#[tauri::command]
pub fn context_save(store: State<'_, Store>, context: Context) -> CommandResult<Context> {
    let archived = store
        .all_custom_layouts()?
        .iter()
        .any(|l| l.archived && l.key() == context.settings.layout);
    if archived {
        return Err("Ein archiviertes Layout kann nicht das Standard-Layout sein.".into());
    }
    if !context.brand.is_empty() && !BRANDS.contains(&context.brand.as_str()) {
        return Err("Diese Farbe gibt es nicht.".into());
    }
    // a pattern that cannot count would stop every export
    Pattern::parse(&context.settings.number_pattern)?;
    store.save_context(&context).map_err(CommandError::from)
}

#[tauri::command]
pub fn context_archive(store: State<'_, Store>, id: i64) -> CommandResult<Context> {
    store.archive_context(id).map_err(CommandError::from)
}

/// What belongs to a context: asked before it is removed or archived
#[tauri::command]
pub fn context_usage(store: State<'_, Store>, id: i64) -> CommandResult<ContextUsage> {
    store.context_usage(id).map_err(CommandError::from)
}

/// Removes a context without invoices, `what` settles its customers, templates and layouts.
#[tauri::command]
pub fn context_remove(store: State<'_, Store>, id: i64, what: Disposition) -> CommandResult<()> {
    store.remove_context(id, what).map_err(CommandError::from)
}

/// A new context to fill in: the defaults of the app, no seller yet
#[tauri::command]
pub fn context_blank() -> Context {
    Context::new(Settings::default(), "").checked()
}

/// What settings as they are being typed still lack, so the form can say so before it is saved
#[tauri::command]
pub fn settings_check(settings: Settings) -> Missing {
    Missing {
        missing: settings.missing(),
        missing_for_xrechnung: settings.missing_for_xrechnung(),
    }
}

/// What the next invoice would be called by a pattern that is being typed, an invoice of today and
/// the first of its kind; the problem if the pattern is none.
#[tauri::command]
pub fn number_example(pattern: String) -> CommandResult<String> {
    let today = crate::utils::issue_day(&crate::utils::today());
    Pattern::parse(&pattern)
        .map(|p| p.next(today, []))
        .map_err(CommandError::from)
}

/// The palettes a context can be tinted with, the first one is the default
#[tauri::command]
pub fn context_brands() -> Vec<&'static str> {
    BRANDS.to_vec()
}
