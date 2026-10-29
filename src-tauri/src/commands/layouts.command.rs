use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use super::preview_invoice_command::Preview;
use crate::draft::{build_invoice, for_preview};

use crate::layout::ImageFile;
use crate::layout_bundle;
use crate::layout_files;
use crate::model::{CustomLayout, Customer, FontFile, InvoiceDraft, Kind, LayoutUsage, Position};
use crate::render::{self, Branding};
use crate::render::{layouts, LayoutInfo};
use crate::sidecar::Sidecars;
use crate::store::Store;
use crate::utils::{today, CommandError, CommandResult};

/// Where the pictures and fonts of own layouts are kept
fn files_dir(app: &AppHandle) -> CommandResult<PathBuf> {
    crate::utils::data_dir(app)
        .map(|dir| dir.join("layouts"))
        .map_err(CommandError::from)
}

/// The invoice layouts a context and its invoice form can choose from: built in, shared and its own.
#[tauri::command]
pub fn layouts_list(store: State<'_, Store>, context_id: i64) -> CommandResult<Vec<LayoutInfo>> {
    Ok(layouts(&store.custom_layouts(context_id)?))
}

/// The own layouts of a context, for the layout settings: shared ones and those only it can use
#[tauri::command]
pub fn layouts_custom(
    store: State<'_, Store>,
    context_id: i64,
) -> CommandResult<Vec<CustomLayout>> {
    store.custom_layouts(context_id).map_err(CommandError::from)
}

#[tauri::command]
pub fn layout_save(store: State<'_, Store>, layout: CustomLayout) -> CommandResult<CustomLayout> {
    save(&store, layout)
}

/// What `layout_save` does with the store
pub(crate) fn save(store: &Store, layout: CustomLayout) -> CommandResult<CustomLayout> {
    if layout.name.trim().is_empty() {
        return Err("Bitte dem Layout einen Namen geben.".into());
    }
    if let Some(color) = layout.invalid_color() {
        return Err(format!("„{color}“ ist keine Farbe, erwartet wird #rrggbb.").into());
    }
    if layout.archived && store.is_default_layout(&layout.key())? {
        return Err("Das Standard-Layout kann nicht archiviert werden.".into());
    }
    store
        .save_custom_layout(&layout)
        .map_err(CommandError::from)
}

/// The invoices made with the layout, drafts included
#[tauri::command]
pub fn layout_usage(store: State<'_, Store>, id: i64) -> CommandResult<Vec<LayoutUsage>> {
    store.layout_usages(id).map_err(CommandError::from)
}

/// Only a layout nothing depends on is deleted: not the default, not used by an invoice (those
/// archive it instead).
#[tauri::command]
pub fn layout_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    if store.is_default_layout(&format!("custom-{id}"))? {
        return Err("Das Standard-Layout kann nicht gelöscht werden.".into());
    }
    if !store.layout_usages(id)?.is_empty() {
        return Err(
            "Das Layout wird von Rechnungen oder Angeboten verwendet, es kann nur archiviert werden.".into(),
        );
    }
    store.delete_custom_layout(id).map_err(CommandError::from)
}

/// Copies a picture for an image part, refuses what is too big or no picture.
#[tauri::command]
pub fn layout_import_image(app: AppHandle, path: PathBuf) -> CommandResult<ImageFile> {
    layout_files::import_image(&path, &files_dir(&app)?).map_err(CommandError::from)
}

/// What a new layout starts as: the built-in one, parts and all.
#[tauri::command]
pub fn layout_builtin() -> CustomLayout {
    crate::layout::builtin()
}

/// Copies a font file, a collection gives one font per face.
#[tauri::command]
pub fn layout_import_font(app: AppHandle, path: PathBuf) -> CommandResult<Vec<FontFile>> {
    layout_files::import_font(&path, &files_dir(&app)?.join("fonts")).map_err(CommandError::from)
}

/// Writes a layout, as it is in the form, with its files into a file the user picked.
#[tauri::command]
pub fn layout_export(
    app: AppHandle,
    layout: CustomLayout,
    with_fonts: bool,
    path: PathBuf,
) -> CommandResult<()> {
    let text = layout_bundle::export(&layout, with_fonts, &files_dir(&app)?)?;
    std::fs::write(path, text).map_err(CommandError::from)
}

/// What the user decided about an import, the answers to what it asked for so far
#[derive(Debug, Default, Deserialize)]
pub struct ImportChoice {
    /// Replace the stored copy of the same layout
    #[serde(default)]
    pub replace: bool,
    /// The name to store it under, instead of the one in the file
    #[serde(default)]
    pub name: Option<String>,
}

/// The stored copy of a layout that a file brings again
#[derive(Debug, Serialize)]
pub struct Stored {
    pub name: String,
    pub updated_at: String,
    /// Invoices and drafts that were made with it
    pub uses: usize,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ImportOutcome {
    /// Stored, with the fonts the file asked for but did not bring
    Done {
        layout: Box<CustomLayout>,
        missing_fonts: Vec<String>,
    },
    /// The layout is stored already: replace it or not, the user decides
    Replace {
        existing: Stored,
        incoming_name: String,
        incoming_updated_at: String,
    },
    /// Another layout has the name: the user names this one
    NameTaken { name: String, suggested: String },
}

/// What an import does with a file, given the layouts there are
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    Create(String),
    /// The id of the layout that is replaced, and the name afterwards
    Replace(i64, String),
    AskReplace(i64),
    AskName(String),
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// The same `uid` is the same layout, replacing it needs a yes. Every name is for one layout only,
/// whatever the `uid`s say.
fn plan(layouts: &[CustomLayout], uid: &str, file_name: &str, choice: &ImportChoice) -> Plan {
    let same = Some(uid)
        .filter(|uid| !uid.is_empty())
        .and_then(|uid| layouts.iter().find(|l| l.uid == uid));
    if let Some(existing) = same.filter(|_| !choice.replace) {
        return Plan::AskReplace(existing.id.unwrap_or_default());
    }
    let name = choice
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| file_name.trim())
        .to_string();
    let target = same.and_then(|l| l.id);
    if layouts
        .iter()
        .any(|l| l.id != target && same_name(&l.name, &name))
    {
        return Plan::AskName(name);
    }
    match target {
        Some(id) => Plan::Replace(id, name),
        None => Plan::Create(name),
    }
}

/// A name that is free and says where the layout came from, not that it follows another one
fn suggest(layouts: &[CustomLayout], name: &str) -> String {
    let now = chrono::Local::now();
    let free = |candidate: &str| !layouts.iter().any(|l| same_name(&l.name, candidate));
    let dated = format!("{name} (Import {})", now.format("%d.%m.%Y"));
    if free(&dated) {
        return dated;
    }
    format!("{name} (Import {})", now.format("%d.%m.%Y %H:%M"))
}

/// Reads a layout file. A layout that is not stored yet is stored right away; one that is stored
/// (same `uid`) is replaced only when the user said so, and a name another layout has is never
/// taken. Until the answers are in nothing is copied or changed, the call comes back asking.
#[tauri::command]
pub fn layout_import(
    app: AppHandle,
    store: State<'_, Store>,
    path: PathBuf,
    choice: Option<ImportChoice>,
) -> CommandResult<ImportOutcome> {
    let size = std::fs::metadata(&path)?.len();
    if size > layout_bundle::MAX_BUNDLE_BYTES {
        return Err("Die Datei ist zu groß für ein Layout.".into());
    }
    let parsed = layout_bundle::parse(&std::fs::read_to_string(&path)?)?;
    let layouts = store.all_custom_layouts()?;
    let find = |id: i64| layouts.iter().find(|l| l.id == Some(id));

    let (name, replacing) = match plan(
        &layouts,
        parsed.uid(),
        parsed.name(),
        &choice.unwrap_or_default(),
    ) {
        Plan::AskReplace(id) => {
            let existing = find(id).ok_or("Das Layout gibt es nicht mehr.")?;
            let uses = store.layout_usages(id)?.len();
            let is_default = store.is_default_layout(&existing.key())?;
            return Ok(ImportOutcome::Replace {
                existing: Stored {
                    name: existing.name.clone(),
                    updated_at: existing.updated_at.clone(),
                    uses,
                    is_default,
                },
                incoming_name: parsed.name().to_string(),
                incoming_updated_at: parsed.updated_at().to_string(),
            });
        }
        Plan::AskName(name) => {
            return Ok(ImportOutcome::NameTaken {
                suggested: suggest(&layouts, &name),
                name,
            })
        }
        Plan::Create(name) => (name, None),
        Plan::Replace(id, name) => (name, find(id)),
    };

    let imported = layout_bundle::install(parsed, Some(&name), &files_dir(&app)?)?;
    let mut layout = imported.layout;
    if let Some(existing) = replacing {
        // the same layout, so what points at it keeps doing so
        layout.id = existing.id;
        layout.context_id = existing.context_id;
        layout.archived = existing.archived;
    }
    Ok(ImportOutcome::Done {
        layout: Box::new(store.store_imported_layout(&layout)?),
        missing_fonts: imported.missing_fonts,
    })
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
            unit: Some("C62".into()),
            unit_label: Some("Stk.".into()),
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
    context_id: Option<i64>,
) -> CommandResult<Preview> {
    // the sample shows the seller of the context the layout is edited for, if there is one yet
    let settings = context_id
        .and_then(|id| store.settings(id).ok())
        .unwrap_or_default();
    let sidecars = Sidecars::locate(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        render_sample(&sidecars, &settings, layout.as_ref(), guides)
    })
    .await?
}

/// The sample invoice in the layout, rendered; what `layout_preview` does without the app
pub(crate) fn render_sample(
    sidecars: &Sidecars,
    settings: &crate::model::Settings,
    layout: Option<&CustomLayout>,
    guides: bool,
) -> CommandResult<Preview> {
    let draft = sample(settings, layout);
    let invoice = match build_invoice(&for_preview(&draft), settings, "RE-BEISPIEL") {
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
        ..Branding::for_document(Kind::Invoice, &draft, settings, &[])
    };
    let dir = tempfile::tempdir()?;
    let (pages, parts) = render::render_sample(sidecars, dir.path(), &invoice, &branding)?;
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

    fn stored(id: i64, uid: &str, name: &str) -> CustomLayout {
        CustomLayout {
            id: Some(id),
            uid: uid.into(),
            name: name.into(),
            ..layout()
        }
    }

    #[test]
    fn an_import_asks_before_it_replaces_or_takes_a_name() {
        let layouts = [stored(1, "a", "Rechnung"), stored(2, "b", "Anna")];
        let choice = |replace, name: Option<&str>| ImportChoice {
            replace,
            name: name.map(Into::into),
        };

        // new layout, free name
        assert_eq!(
            plan(&layouts, "c", "Neu", &choice(false, None)),
            Plan::Create("Neu".into())
        );
        // no uid (the built-in layout) is always a new one
        assert_eq!(
            plan(&layouts, "", "Neu", &choice(false, None)),
            Plan::Create("Neu".into())
        );
        // another layout that has the name, whatever the case, however renamed
        assert_eq!(
            plan(&layouts, "c", " anna ", &choice(false, None)),
            Plan::AskName("anna".into())
        );
        assert_eq!(
            plan(&layouts, "c", "Anna", &choice(false, Some("Anna 2"))),
            Plan::Create("Anna 2".into())
        );
        // the same layout again: asks, then replaces, under its own name
        assert_eq!(
            plan(&layouts, "a", "Rechnung", &choice(false, None)),
            Plan::AskReplace(1)
        );
        assert_eq!(
            plan(&layouts, "a", "Rechnung", &choice(true, None)),
            Plan::Replace(1, "Rechnung".into())
        );
        // renamed in the file to a name of another layout: that is asked as well
        assert_eq!(
            plan(&layouts, "a", "Anna", &choice(true, None)),
            Plan::AskName("Anna".into())
        );
        assert_eq!(
            plan(&layouts, "a", "Anna", &choice(true, Some("Rechnung neu"))),
            Plan::Replace(1, "Rechnung neu".into())
        );

        // the suggestion is free and does not look like a successor
        let suggested = suggest(&layouts, "Anna");
        assert!(suggested.starts_with("Anna (Import "));
        assert!(!layouts.iter().any(|l| same_name(&l.name, &suggested)));
    }

    #[test]
    fn the_sample_is_an_invoice_that_can_be_built_and_wears_the_layout() {
        let draft = sample(&settings(), Some(&layout()));

        assert!(build_invoice(&for_preview(&draft), &settings(), "RE-BEISPIEL").is_ok());
        assert!(draft.positions.len() >= 30, "long enough for a second page");
        let branding = Branding::for_document(Kind::Invoice, &draft, &settings(), &[]);
        assert_eq!(branding.custom.map(|c| c.name).as_deref(), Some("Neu"));

        // without a layout the sample shows the built-in one, whatever the default is
        let plain = sample(&settings(), None);
        assert!(
            Branding::for_document(Kind::Invoice, &plain, &settings(), &[])
                .custom
                .is_none()
        );
    }
}
