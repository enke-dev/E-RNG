use tauri::State;

use crate::context::{Context, ContextUsage, Disposition, Missing, BRANDS};
use crate::model::{Kind, Settings};
use crate::numbering::{reset_problem, Calendar, Pattern, Reset};
use crate::store::{Sequence, Store};
use crate::units::UnitDef;
use crate::utils::{issue_day, today, CommandError, CommandResult};

/// Every context with what is missing in it, archived ones included (the UI hides them)
#[tauri::command]
pub fn contexts_list(store: State<'_, Store>) -> CommandResult<Vec<Context>> {
    store.contexts().map_err(CommandError::from)
}

/// Creates (`id` is `None`) or updates a context. An incomplete one is saved as well, the data is
/// filled in step by step, `missing` tells what is still to do. A new context starts with `units`
/// as its list of units (none: the defaults); an existing one keeps its own.
#[tauri::command]
pub fn context_save(
    store: State<'_, Store>,
    mut context: Context,
    units: Option<Vec<UnitDef>>,
) -> CommandResult<Context> {
    // what a paste brings along ends up in the XML and the PDF, so it goes before saving
    context.settings.seller = context.settings.seller.cleaned();
    let archived = store.all_custom_layouts()?.iter().any(|l| {
        l.archived
            && [
                Some(context.settings.layout.as_str()),
                context.settings.offer.layout.as_deref(),
            ]
            .contains(&Some(l.key().as_str()))
    });
    if archived {
        return Err("Ein archiviertes Layout kann nicht das Standard-Layout sein.".into());
    }
    if !context.brand.is_empty() && !BRANDS.contains(&context.brand.as_str()) {
        return Err("Diese Farbe gibt es nicht.".into());
    }
    // a pattern that cannot count would stop every export
    Pattern::parse(&context.settings.number_pattern)?;
    Pattern::parse(&context.settings.offer.number_pattern)?;
    if let Some(problem) = [Kind::Invoice, Kind::Offer]
        .into_iter()
        .find_map(|kind| context.settings.reset_problem(kind))
    {
        return Err(problem.into());
    }
    store
        .save_context(&context, units.as_deref())
        .map_err(CommandError::from)
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
        missing_for_offers: settings.missing_for_offers(),
    }
}

/// What the next invoice would be called by a pattern that is being typed, an invoice of today and
/// the first of its period; the problem if the pattern is none or does not tell the periods of
/// the reset apart.
#[tauri::command]
pub fn number_example(
    pattern: String,
    reset: Reset,
    fiscal_year_start: Option<en16931::Date>,
) -> CommandResult<String> {
    let calendar = Calendar { fiscal_year_start };
    let today = issue_day(&today());
    let pattern = Pattern::parse(&pattern)?;
    match reset_problem(&pattern, reset, calendar, today.year()) {
        Some(problem) => Err(problem.into()),
        None => Ok(pattern.number(today, calendar, 1)),
    }
}

/// Where the numbers of a kind of a context stand today, by its saved settings
#[tauri::command]
pub fn number_sequence(
    store: State<'_, Store>,
    context_id: i64,
    kind: Kind,
) -> CommandResult<Sequence> {
    let settings = store.settings(context_id)?;
    store
        .sequence(context_id, kind, &settings, issue_day(&today()))
        .map_err(CommandError::from)
}

/// Continues the numbers of today's period from `next`, saved at once; refused with the reason
/// when that number or a higher one is taken already.
#[tauri::command]
pub fn number_sequence_set(
    store: State<'_, Store>,
    context_id: i64,
    kind: Kind,
    next: u32,
) -> CommandResult<Sequence> {
    let settings = store.settings(context_id)?;
    store
        .set_counter(context_id, kind, &settings, issue_day(&today()), next)
        .map_err(CommandError::from)
}

/// The palettes a context can be tinted with, the first one is the default
#[tauri::command]
pub fn context_brands() -> Vec<&'static str> {
    BRANDS.to_vec()
}
