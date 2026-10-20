use serde::Serialize;
use tauri::{AppHandle, State};

use crate::draft::{build_invoice, for_preview, InputProblem};
use crate::model::InvoiceDraft;
use crate::render::{self, Branding};
use crate::sidecar::Sidecars;
use crate::store::Store;
use crate::utils::{issue_day, message};

#[derive(Debug, Serialize)]
pub struct Preview {
    /// One SVG document per page, as the template lays the invoice out
    pub pages: Vec<String>,
    /// Set instead of pages when even a half filled form cannot be shown
    pub problems: Vec<InputProblem>,
    /// Where the parts of the layout are on the pages, only for the editor of a layout
    pub parts: Vec<crate::render::PartBox>,
}

/// The invoice as it will look, rendered by the real template for the form's current state.
/// Nothing is stored, a half filled form is shown with stand-ins for what is missing.
#[tauri::command]
pub async fn preview_invoice(
    app: AppHandle,
    store: State<'_, Store>,
    draft: InvoiceDraft,
    id: Option<i64>,
) -> Result<Preview, String> {
    let current = store.settings().map_err(message)?;
    let settings = draft.frozen.clone().unwrap_or(current.clone());

    // an exported invoice shows its number, a draft the one it would get
    let stored_number = id
        .map(|id| store.invoice(id))
        .transpose()
        .map_err(message)?
        .and_then(|invoice| invoice.number);
    let number = match stored_number {
        Some(number) => number,
        None => store
            .next_number(&current.number_pattern, issue_day(&draft.issue_date))
            .map_err(message)?,
    };

    let invoice = match build_invoice(&for_preview(&draft), &current, &number) {
        Ok(invoice) => invoice,
        Err(problems) => {
            return Ok(Preview {
                pages: Vec::new(),
                problems,
                parts: Vec::new(),
            })
        }
    };
    let customs = store.custom_layouts().map_err(message)?;
    let branding = Branding::for_draft(&draft, &settings, &customs);
    let sidecars = Sidecars::locate(&app).map_err(message)?;

    let pages = tauri::async_runtime::spawn_blocking(move || {
        let dir = tempfile::tempdir()?;
        render::render_preview(&sidecars, dir.path(), &invoice, &branding)
    })
    .await
    .map_err(message)?
    .map_err(message)?;
    Ok(Preview {
        pages,
        problems: Vec::new(),
        parts: Vec::new(),
    })
}
