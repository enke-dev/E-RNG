use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::draft::{build_invoice, for_preview, InputProblem};
use crate::invoice::Invoice;
use crate::model::InvoiceDraft;
use crate::numbering::Pattern;
use crate::render::{self, Branding};
use crate::sidecar::Sidecars;
use crate::store::Store;
use crate::utils::{issue_day, CommandError, CommandResult};

#[derive(Debug, Serialize)]
pub struct Preview {
    /// One SVG document per page, as the template lays the invoice out
    pub pages: Vec<String>,
    /// Set instead of pages when even a half filled form cannot be shown
    pub problems: Vec<InputProblem>,
    /// Where the parts of the layout are on the pages, only for the editor of a layout
    pub parts: Vec<crate::render::PartBox>,
}

/// The invoice the form makes now, with stand-ins for what is missing, and how it looks. A draft
/// is shown as one: its title says so and the number is only the pattern (`RE-2026-###`), it gets
/// a real one when it is created.
fn shown(
    store: &Store,
    draft: &InvoiceDraft,
    id: Option<i64>,
    context_id: i64,
) -> CommandResult<Result<(Invoice, Branding), Vec<InputProblem>>> {
    let current = store.settings(context_id)?;
    let settings = draft.frozen.clone().unwrap_or(current.clone());

    let stored_number = id
        .map(|id| store.invoice(id))
        .transpose()?
        .and_then(|invoice| invoice.number);
    let is_draft = stored_number.is_none();
    let number = match stored_number {
        Some(number) => number,
        None => Pattern::parse(&current.number_pattern)?.masked(issue_day(&draft.issue_date)),
    };

    let customs = store.all_custom_layouts()?;
    Ok(
        build_invoice(&for_preview(draft), &current, &number).map(|invoice| {
            let branding = Branding {
                draft: is_draft,
                ..Branding::for_draft(draft, &settings, &customs)
            };
            (invoice, branding)
        }),
    )
}

/// The invoice as it will look, rendered by the real template for the form's current state.
/// Nothing is stored, a half filled form is shown with stand-ins for what is missing.
#[tauri::command]
pub async fn preview_invoice(
    app: AppHandle,
    store: State<'_, Store>,
    draft: InvoiceDraft,
    id: Option<i64>,
    context_id: i64,
) -> CommandResult<Preview> {
    let (invoice, branding) = match shown(&store, &draft, id, context_id)? {
        Ok(shown) => shown,
        Err(problems) => {
            return Ok(Preview {
                pages: Vec::new(),
                problems,
                parts: Vec::new(),
            })
        }
    };
    let sidecars = Sidecars::locate(&app)?;

    let pages = tauri::async_runtime::spawn_blocking(move || {
        let dir = tempfile::tempdir()?;
        render::render_preview(&sidecars, dir.path(), &invoice, &branding)
    })
    .await??;
    Ok(Preview {
        pages,
        problems: Vec::new(),
        parts: Vec::new(),
    })
}

/// The draft as a plain PDF, the same as the preview: to show it around before it is created. It
/// says it is a draft, carries no number and no XML, and is not checked.
#[tauri::command]
pub async fn invoice_draft_pdf(
    app: AppHandle,
    store: State<'_, Store>,
    draft: InvoiceDraft,
    id: Option<i64>,
    context_id: i64,
    target: PathBuf,
) -> CommandResult<()> {
    let (invoice, branding) = shown(&store, &draft, id, context_id)?.map_err(|problems| {
        let messages: Vec<_> = problems.into_iter().map(|p| p.message).collect();
        CommandError::from(messages.join(", "))
    })?;
    if !branding.draft {
        return Err("Eine erstellte Rechnung wird über „Exportieren als“ gespeichert.".into());
    }
    let sidecars = Sidecars::locate(&app)?;

    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let pdf = render::render_pdf(&sidecars, dir.path(), &invoice, &branding)?;
        std::fs::copy(pdf, &target)?;
        Ok(())
    })
    .await??;
    Ok(())
}
