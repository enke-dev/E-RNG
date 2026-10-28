# Configurable units – Plan

## Review decisions (2026-10-29) – they override the text below

- **Units are per context**, not global. The `units` table gets `context_id NOT NULL`; each context has its own list, seeded with Stk. (C62), Std. (HUR), pauschal (LS). A new context starts with a copy of the active context's units. The settings show "Einheiten" in each context's group (not in a global "Allgemein" group); the position selects show the units of the invoice's context.
- **Migration keeps what is in use**: per context, the defaults plus every unit its own drafts and its scoped saved positions use; units of shared saved positions (`context_id` NULL) are added to every context, so no shared position becomes an orphan anywhere.
- **Rename and code change follow silently** in drafts and saved positions of that context (and shared saved positions, which then may show an orphan in other contexts – the select shows it as "nicht in der Liste"), the dialog says how many; created invoices keep their label (as proposed).
- Several labels per code: allowed. A duplicated invoice whose unit was deleted keeps the old label. The catalog as proposed. (Open questions 1–6 closed.)

Goal: positions get their unit from a list the user maintains (a label plus a UN/ECE rec. 20 code) instead of the hard-coded `UNITS` in `src/common/utils/format.utils.ts` and `unit_name()` in `src-tauri/src/render/view.rs`. The app only ships three defaults: **Stk.** (C62), **Std.** (HUR), **pauschal** (LS). Units can be renamed and deleted without changing any created invoice.

Status: plan, not started. Related: [import-historic.md](import-historic.md) (imported positions bring codes that may not be in the list).

## How it works today (for reference)

- `Position.unit: Option<String>` (`model.rs`) holds the code; empty means C62. `Position::same_as` compares codes.
- The frontend offers `UNITS` (C62 Stk., HUR Std., DAY Tag(e), MON Monat(e), LS pauschal) in `invoice-edit.view.ts` and `templates.view.ts`. Each of those files resolves labels with its own `unitLabel()`.
- The document gets its label from `unit_name(code)` in `render/view.rs`. That list is wider (also WEE, KGM, LTR, MTR, KWH, and H87 → "Stk."). The XML only carries the code (`line_from_unit_price`, `draft.rs::line`).
- A created invoice keeps its frozen settings and layout, but the unit label is looked up again on every render. That includes the locked view's preview (`preview_invoice`) and regenerating legacy invoices.

## Decisions

- **The label is copied into the position.** `Position` gets `unit_label: Option<String>` next to the code. Picking a unit writes both. This works like the customer copy in the draft: a created invoice carries what it showed, so deleting or renaming a unit can never change it, and nothing new needs freezing. The XML still gets only the code.
- **Old data is filled in when read, never rewritten.** A position without `unit_label` gets the label the old `unit_name()` would have printed (`units::legacy_label`, a frozen copy of that table that is never edited). The same happens for an empty code (C62 "Stk."). Created invoices therefore look exactly as before, and their rows are not touched. This follows the `Settings::normalized()` approach ("becomes … when settings are read").
- **Global, not per context.** Units are vocabulary, not issuer data. Saved positions are shared by default and must find their unit in every context. Because positions carry code and label, a later per-context scope would not need to migrate positions (open question 1).
- **Several labels may share a code** ("Stk." and "Lizenz" both C62, "Seite" as ZP or C62). Labels are unique, ignoring case. A unit is identified by its row `id` in the list. Positions match a unit by (code, label).
- **Picking the code: a curated list plus a checked free entry.** `units::CATALOG` holds about 30 common rec. 20 codes with German names: C62 Stück, H87 Stück (Teil), HUR Stunde, MIN Minute, DAY Tag, E49 Arbeitstag, WEE Woche, MON Monat, ANN Jahr, LS Pauschale, E48 Serviceeinheit, SET Satz, PR Paar, XPK Packung, ZP Seite, P1 Prozent, KGM kg, GRM g, TNE t, MTR m, CMT cm, KMT km, MTK m², MTQ m³, LTR l, MLT ml, KWH kWh. "Anderer Code…" accepts any code that `en16931::codes::guard::unit` accepts, and shows that function's hint for things like `kwh` or a stray space. A test checks every catalog code against the same function. A full name list of about 2000 codes is not worth carrying.
- **Delete:**
  - A unit that no saved position and no open draft uses is deleted after a plain `ask`.
  - A used unit opens a dialog that lists where it is used and asks for the replacement unit (required). Saved positions and drafts are rewritten to the replacement in one transaction.
  - Created, imported and archived invoices are never touched.
  - The last unit cannot be deleted.
- **Rename and code change apply to drafts and saved positions at once.** The edit dialog says so ("Wird in 3 gespeicherten Positionen und 2 Entwürfen verwendet, sie übernehmen die Änderung."). Created invoices keep the old label.
- **Order is the list order**, changed by dragging (`reorder.utils.ts`). A new position starts with the first unit. `unit: null` is no longer written by the UI. It is still read as C62 "Stk.".
- **A unit missing from the list** (an orphan from a duplicated old invoice, a deleted unit, an import) still shows in the position's select as an extra disabled option "‹label› (nicht in der Liste)". It still renders and can still be created; the user can switch it.
- **Settings UI:** a new list group **"Allgemein"** in the settings sidebar, placed between the contexts and "Layouts", with one item **"Einheiten"** (sub line "Für alle Kontexte"), route `/settings/units`. Each change is saved in a dialog right away, so the page needs no leave check.

## Data model

Migration **v7** (`store.rs`, `migrate_to_units`):

```sql
CREATE TABLE IF NOT EXISTS units (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL,
    label TEXT NOT NULL,
    sort INTEGER NOT NULL
);
```

- Seed with Stk./C62, Std./HUR, pauschal/LS (sort 0, 1, 2).
- Then scan `templates.json` and the `invoices.draft` of rows with `status = 'draft'`. Each (code, legacy label) they use that is not seeded yet is added in order of first use. Example: a template with DAY adds "Tag(e)"/DAY, so nothing open becomes an orphan.
- Exported invoices are not scanned. They resolve through `legacy_label` and are never edited.
- Label uniqueness is checked in Rust (case and spaces aside), not with a SQL constraint.

`model.rs`:

```rust
pub struct Position {
    pub description: String,
    pub quantity: Option<String>,
    /// UN/ECE rec. 20 code, empty means C62 (older data)
    pub unit: Option<String>,
    /// The label as it was picked; empty (older data): what the code was printed as then
    #[serde(default)]
    pub unit_label: Option<String>,
    pub price: String,
}
impl Position {
    pub fn unit_code(&self) -> &str;          // blank → "C62"
    pub fn unit_label(&self) -> String;       // own label, else units::legacy_label(code)
    pub fn normalized(self) -> Self;          // fills both, used when rows are read
}
```

`Position::same_as` compares `unit_code()` and `unit_label()`, so "Std." and "Stunde" on HUR are different saved positions.

New `src-tauri/src/units.rs`:

```rust
#[derive(Serialize, Deserialize)]
pub struct Unit { pub id: Option<i64>, pub code: String, pub label: String }
pub struct CatalogEntry { pub code: &'static str, pub name: &'static str }
pub const CATALOG: &[CatalogEntry];
pub const DEFAULTS: [(&str, &str); 3];          // ("C62","Stk."), ("HUR","Std."), ("LS","pauschal")
pub fn legacy_label(code: &str) -> String;      // the old unit_name table, verbatim, never edited
pub fn catalog_name(code: &str) -> Option<&'static str>;
pub fn check(unit: &Unit, others: &[Unit]) -> Result<Unit>; // trims, guard::unit, unique label (German messages)
#[derive(Serialize)]
pub struct UnitUsage { pub templates: Vec<TemplateRef>, pub drafts: Vec<DraftRef> }
// TemplateRef { id, description, context_id }, DraftRef { invoice_id, customer, context_id }
```

## Backend (src-tauri)

- `store.rs`:
  - The `migrate` step for v7, and `SCHEMA_VERSION = 7`.
  - `units() -> Vec<Unit>` (ordered by `sort`), `save_unit(&Unit) -> Unit`, `unit_usage(id) -> UnitUsage`, `delete_unit(id, replacement: Option<i64>)`, `reorder_units(&[i64])`.
  - The rewrite is a private `rewrite_positions(tx: &Connection, from: (&str, &str), to: (&str, &str)) -> Result<usize>`. It goes through `templates` and `invoices WHERE status = 'draft'`, matches on `(unit_code(), unit_label())`, and writes the JSON back. It does not change `updated_at`, so the list order stays. `save_unit` uses it when code or label changed; `delete_unit` uses it with the replacement.
  - It holds one connection and runs one `unchecked_transaction()`, like `migrate_to_contexts`. Do not call `self.templates()` while holding the lock.
  - `delete_unit` refuses a used unit when no replacement is given, the last unit, and a replacement that is the unit itself.
  - `invoice()`, `templates()` and `all_templates()` return `normalized()` positions, so the UI always gets a code and a label.
- `commands/units.command.rs`: `units_list`, `units_catalog`, `unit_save`, `unit_usage`, `unit_delete { id, replacement }`, `units_reorder { ids }`. Register them in `lib.rs` and the `commands/mod.rs` path list.
- `render/view.rs`:
  - `Branding` gets `unit_labels: Vec<String>`, one per position. `Branding::for_draft` fills it from `draft.positions` (`Position::unit_label`). The signature does not change, so no caller changes.
  - `line_views` takes `branding.unit_labels.get(i)` (lines are 1:1 with positions, `draft.rs::line` numbers them `index + 1`, and `for_preview` keeps the count). The fallback is `units::catalog_name(code)`, then the code. The fallback is for `Branding { .. }` literals in tests and the sample.
  - Delete `unit_name`. The existing `Branding { … }` literals in `pipeline.rs`, `render/mod.rs` and `layouts.command.rs` need `unit_labels: Vec::new()` or `..Branding::default()`.
- `commands/layouts.command.rs::sample`: positions with `unit: Some("C62")`, `unit_label: Some("Stk.")`.
- `draft.rs`: no change. The XML keeps `position.unit_code()`. Optionally add an `InputProblem` on `positions[i].unit` when `guard::unit` refuses a code (only possible through import or hand-edited data).

## Frontend (src)

- `api.utils.ts`:
  - `Position.unit_label: string | null`.
  - The types `Unit`, `CatalogEntry`, `UnitUsage`.
  - The calls `unitsList`, `unitsCatalog`, `unitSave`, `unitUsage`, `unitDelete(id, replacement)`, `unitsReorder(ids)`.
- New `src/common/utils/units.utils.ts`:
  - A `units` signal, loaded like `contexts` in `context.utils.ts`, with `ensureUnits()` and `reloadUnits()`.
  - `unitOf(position)`: finds the list entry by code and label.
  - `unitLabel(position)` and `pick(position, unit)`: `pick` writes code and label.
  - Delete `UNITS` from `format.utils.ts` and both local `unitLabel` functions.
- `invoice-edit.view.ts`:
  - The unit `wa-select` value is the unit `id`, with the orphan option when needed.
  - A new position gets the first unit.
  - A template inserted from the picker (around line 506) copies `unit_label`. `saveAsTemplate` already copies the whole position.
- `templates.view.ts`: the edit dialog uses the same select. The unit filter goes by unit id and compares code and label. Cards show `unitLabel(t)`.
- `settings.view.ts`:
  - In `renderList()`, a group `<erng-list-group heading="Allgemein">` with `this.item('units', 'Einheiten', 'Für alle Kontexte')`.
  - In `renderDetail()`, `if (this.section === 'units') return html`<erng-unit-list slot="detail">`;`.
  - The page goes into a component, so the 1500-line view does not grow: `src/common/components/unit-list/unit-list.component.ts` (+ `.css`) inside `<erng-page>`. It holds:
    - Rows: drag handle, label, code with the catalog name (muted), and a usage count.
    - **"Einheit hinzufügen"** in the `actions` slot.
    - A row click opens **edit dialog**: "Bezeichnung", then "Code" as a `wa-select` of `CATALOG` ("HUR – Stunde") plus "Anderer Code…", which shows a `wa-input`. Left: "Abbrechen". Right: "Löschen" (danger), "Speichern" (brand).
    - **Delete dialog** (only when used): lists the saved positions and drafts (customer, context), and asks for a required "Ersetzen durch" select. Left: "Abbrechen". Right: "Ersetzen und löschen" (danger).
  - After each change, call `reloadUnits()` and `notify`.
- Open drafts and the units page are never open together (one window, routes). A draft saves itself when the view is left, so the rewrite in the store sees its latest state.
- `mock.utils.ts`: an in-memory unit list with the three defaults plus the commands.

## Tests

- Rust (`store.rs`):
  - Migration v6 → v7 on a database that has a template with `DAY` and a draft with `MON`: units are Stk., Std., pauschal, Tag(e), Monat(e). An exported invoice with `WEE` adds nothing and still reads as "Woche(n)".
  - Legacy positions are read with `unit_label` filled; an empty code reads as C62 "Stk.".
  - Delete without usage. A used unit without a replacement is refused. With a replacement, templates and drafts are rewritten, while an exported invoice and an archived one keep code and label byte for byte.
  - Renaming applies to drafts and templates but not to exported invoices.
  - A duplicate label (case aside) is refused, a shared code is allowed, and the last unit cannot be deleted.
  - `same_as` tells two labels on one code apart.
- Rust (`units.rs`): every `CATALOG` code passes `guard::unit`. `kwh` is refused with the hint. `legacy_label` gives exactly the old table (H87 → Stk., DAY → Tag(e), unknown → code).
- Rust (`render/view.rs`): a position "Seite" on C62 prints "Seite" while the CII XML has `unitCode="C62"`. Without labels the catalog name is used.
- Golden sidecar tests (ignored by default) still pass. The sample still shows "Stk.".

## Manual checks

- In Settings → Allgemein → Einheiten: add "Lizenz" (C62), add "Tag" (DAY), reorder, rename "Std." to "Stunde".
- An open draft and a saved position follow the rename. A created invoice still shows "Std." in the form, the preview and "Exportieren als".
- Delete "Stunde" while a draft uses it: the dialog lists the draft and asks for a replacement.
- Duplicate an old invoice that used a deleted unit: the select shows the orphan, and creating still works.
- Enter `kwh` as a code: refused with the hint.

## Later

- Units per context (`context_id` like customers, using the existing scope select).
- Plural or quantity-dependent labels ("Tag" / "Tage").
- A default quantity or price per unit.
- Export and import of the list.
- The full rec. 20 list with names, searchable.

## Open questions

1. Global units are proposed. Is there a case where two contexts need different labels for the same thing?
2. The new defaults are only Stk., Std. and pauschal. Existing users keep what their drafts and templates use (e.g. "Tag(e)", "Monat(e)"). Should Tag and Monat be offered as one-click additions from the catalog instead?
3. Should several labels per code be allowed? (Proposed: yes.)
4. Should renaming silently update drafts and saved positions? (Proposed: yes, with the count shown in the dialog.)
5. Should a duplicated invoice whose unit was deleted keep the old label (proposed) or switch to the first unit?
6. Is the catalog above the right set, or is something missing from your trade (e.g. "Seite", "Wort", "km")?
