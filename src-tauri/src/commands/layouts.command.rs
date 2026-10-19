use std::path::PathBuf;

use tauri::{AppHandle, State};

use super::preview_invoice_command::Preview;
use crate::draft::{build_invoice, for_preview};

use crate::layout::ImageFile;
use crate::layout_files;
use crate::model::{CustomLayout, Customer, FontFile, InvoiceDraft, LayoutUsage, Position};
use crate::render::{self, Branding};
use crate::render::{layouts, LayoutInfo};
use crate::sidecar::Sidecars;
use crate::store::Store;
use crate::utils::{message, today};

/// Where the pictures and fonts of own layouts are kept
fn files_dir(app: &AppHandle) -> Result<PathBuf, String> {
    crate::utils::data_dir(app)
        .map(|dir| dir.join("layouts"))
        .map_err(message)
}

/// The invoice layouts the settings and the invoice form can choose from: built in and own.
#[tauri::command]
pub fn layouts_list(store: State<'_, Store>) -> Result<Vec<LayoutInfo>, String> {
    Ok(layouts(&store.custom_layouts().map_err(message)?))
}

#[tauri::command]
pub fn layouts_custom(store: State<'_, Store>) -> Result<Vec<CustomLayout>, String> {
    store.custom_layouts().map_err(message)
}

#[tauri::command]
pub fn layout_save(store: State<'_, Store>, layout: CustomLayout) -> Result<CustomLayout, String> {
    if layout.name.trim().is_empty() {
        return Err("Bitte dem Layout einen Namen geben.".into());
    }
    if layout.archived && store.settings().map_err(message)?.layout == layout.key() {
        return Err("Das Standard-Layout kann nicht archiviert werden.".into());
    }
    store.save_custom_layout(&layout).map_err(message)
}

/// The invoices made with the layout, drafts included
#[tauri::command]
pub fn layout_usage(store: State<'_, Store>, id: i64) -> Result<Vec<LayoutUsage>, String> {
    store.layout_usages(id).map_err(message)
}

/// Only a layout nothing depends on is deleted: not the default, not used by an invoice (those
/// archive it instead).
#[tauri::command]
pub fn layout_delete(store: State<'_, Store>, id: i64) -> Result<(), String> {
    if store.settings().map_err(message)?.layout == format!("custom-{id}") {
        return Err("Das Standard-Layout kann nicht gelöscht werden.".into());
    }
    if !store.layout_usages(id).map_err(message)?.is_empty() {
        return Err(
            "Das Layout wird von Rechnungen verwendet, es kann nur archiviert werden.".into(),
        );
    }
    store.delete_custom_layout(id).map_err(message)
}

/// Copies a picture for an image part, refuses what is too big or no picture.
#[tauri::command]
pub fn layout_import_image(app: AppHandle, path: PathBuf) -> Result<ImageFile, String> {
    layout_files::import_image(&path, &files_dir(&app)?).map_err(message)
}

/// What a new layout starts as: the built-in one, parts and all.
#[tauri::command]
pub fn layout_builtin() -> CustomLayout {
    crate::layout::builtin()
}

#[tauri::command]
pub fn layout_import_font(app: AppHandle, path: PathBuf) -> Result<FontFile, String> {
    layout_files::import_font(&path, &files_dir(&app)?.join("fonts")).map_err(message)
}

/// A sample invoice long enough for a second page, the same for every layout so they can be compared
fn sample(settings: &crate::model::Settings, layout: Option<&CustomLayout>) -> InvoiceDraft {
    let mut draft = InvoiceDraft::blank(&today());
    draft.customer = Customer {
        name: "Beispiel Kunde GmbH".into(),
        address: crate::model::Address {
            street: "Musterweg 1".into(),
            post_code: "12345".into(),
            city: "Musterstadt".into(),
            country: "DE".into(),
        },
        ..Customer::default()
    };
    draft.positions = (1..=30)
        .map(|n| Position {
            description: format!("Beispielposition {n}"),
            quantity: Some(n.to_string()),
            unit: None,
            price: "12.50".into(),
        })
        .collect();
    if settings.intro.is_none() {
        draft.intro = Some(
            "Sehr geehrte Damen und Herren,\nvielen Dank für Ihren Auftrag, ich stelle Ihnen folgende Leistungen in Rechnung."
                .into(),
        );
    }
    if settings.closing.is_none() {
        draft.closing = Some("Vielen Dank für Ihr Vertrauen.".into());
    }
    // the layout as it is in the form, stored or not; none is the built-in one
    draft.frozen_layout = layout.cloned();
    draft
}

/// The sample invoice in the layout as it is being edited (none: the built-in layout), nothing is
/// stored.
#[tauri::command]
pub async fn layout_preview(
    app: AppHandle,
    store: State<'_, Store>,
    layout: Option<CustomLayout>,
    guides: bool,
) -> Result<Preview, String> {
    let settings = store.settings().map_err(message)?;
    let draft = sample(&settings, layout.as_ref());
    let invoice = match build_invoice(&for_preview(&draft), &settings, "RE-BEISPIEL") {
        Ok(invoice) => invoice,
        Err(problems) => {
            return Ok(Preview {
                pages: Vec::new(),
                problems,
                parts: Vec::new(),
            })
        }
    };
    let branding = Branding {
        guides,
        ..Branding::for_draft(&draft, &settings, &[])
    };
    let sidecars = Sidecars::locate(&app).map_err(message)?;

    let (pages, parts) = tauri::async_runtime::spawn_blocking(move || {
        let dir = tempfile::tempdir()?;
        render::render_sample(&sidecars, dir.path(), &invoice, &branding)
    })
    .await
    .map_err(message)?
    .map_err(message)?;
    Ok(Preview {
        pages,
        problems: Vec::new(),
        parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::fixtures::settings;

    fn layout() -> CustomLayout {
        CustomLayout {
            id: None,
            name: "Neu".into(),
            fonts: Vec::new(),
            ..crate::layout::builtin()
        }
    }

    #[test]
    fn the_sample_is_an_invoice_that_can_be_built_and_wears_the_layout() {
        let draft = sample(&settings(), Some(&layout()));

        assert!(build_invoice(&for_preview(&draft), &settings(), "RE-BEISPIEL").is_ok());
        assert!(draft.positions.len() >= 30, "long enough for a second page");
        let branding = Branding::for_draft(&draft, &settings(), &[]);
        assert_eq!(branding.custom.map(|c| c.name).as_deref(), Some("Neu"));

        // without a layout the sample shows the built-in one, whatever the default is
        let plain = sample(&settings(), None);
        assert!(Branding::for_draft(&plain, &settings(), &[])
            .custom
            .is_none());
    }
}
