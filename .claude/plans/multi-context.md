# Multi-context – Plan

Goal: one install serves several issuers, called **contexts**. A context is a company today, later maybe a Verein or a private person (hence the neutral name, no "company" in code or UI strings). A switch in the header picks the active context. Everything that creates or numbers an invoice runs under exactly one context; the list can optionally show all.

Status: implemented 2026-10-20 (backend with tests, frontend checked by type check and lint, not yet clicked through in the app). The brand color is picked next to the name field with a small dropdown of colored squares (`erng-brand-select`, names only as tooltips); a new context starts with a color no other has. When the invoice list's context filter changes and the selected invoice is not in the new list, the newest one is selected. Switching context keeps the page in the settings (the same page of the other context, a layout stays open or falls back to the built-in one if it belongs to another context). A new invoice has "Abbrechen" and a locked one "Schließen" in its footer, both go to `/invoices/none` (nothing open, the placeholder shows; cancelling asks first when something was typed and saves no draft). The invoice list shows a placeholder with "Neue Rechnung" instead of a blank form while there is no invoice to show. The context select in the header and the "Aktiv" tag in the settings only show with two or more contexts. The settings list has no "Kontexte" heading: every context is a group titled with the seller name, only the open one shows its pages. The active one carries an "Aktiv" tag, the others a grey "Wechseln" button that switches the context (the same switch as the header select, `switchContext`) and so opens its pages. A second context is added with a quiet "+ Kontext hinzufügen" entry after the last context group in the settings; the form opens as an italic placeholder group "Neuer Kontext" with an "Abbrechen" (as long as another context exists to go back to). Leaving the settings with unsaved edits asks leave or stay (`leave.utils.ts`, also on the context switch). Differences from this plan: the settings routes are single segment `context-<id>-<page>` (`context-new-<page>` for a new one) instead of `/settings/contexts/:id`; a new context starts from the active one's tax, numbering and text defaults but with an empty seller; removing a context needs it to have no invoices at all (drafts included), not only no exported ones. Build this **before** [steuerberater-export.md](steuerberater-export.md): the export filters and settings are per context.

## Decisions

- **Strict separation for writes**, mixed view for reads only. Seller, IBAN, VAT id, number pattern and counter differ per context, so create/edit/number/export always have one context.
- **Customers, position templates and layouts: shared by default, optionally scoped to one context.** "Scoped" = the entry belongs to one context and only shows there (an `only for <context>` option in the editor); not scoped = visible in all. Pickers show shared plus the active context's own.
- Each context has a **default layout** (a setting of the context). Contexts may share a CI, so layouts are not per context by default.
- **Context settings are per context**: the whole `Settings` struct moves into the context (seller, tax rate, VAT note, payment days, number pattern/prefix, footer, layout, intro, closing). UI preferences and other app state stay global.
- **Brand color per context**: used by our UI only (not invoices). It is a Web Awesome palette name, stored in the context and applied as a class on `<html>`: `wa-brand-<hue>` re-derives every brand token (`--wa-color-brand-05` … `-95`, `-brand`, `-brand-on`, and from them all `-fill-*`/`-border-*`/`-on-*` semantic tokens) in light and dark mode, so nothing else is set by hand. Presets: blue, red, orange, yellow, green, cyan, indigo, purple, pink (not gray). The first context is set to `wa-brand-blue` explicitly (today blue is only the unclassed default), new contexts get a random preset not yet in use. Check yellow for readability when building. The tint on the whole UI is what shows the active context; no extra badge needed.
- **Active context is remembered** across restarts, device state in the frontend like the last route (see Settled). A draft keeps the context it was created under: switching the header while editing does not change it.
- **Archive, never delete, once invoices exist**:
  - A context with exported invoices can only be archived (timestamp, hidden from the switch, still part of "all" and of exports).
  - Exported invoices can only be archived as well (new: today there is no archive for invoices). Drafts stay deletable.
  - No archive view for now, where it fits in the UI is undecided.
- **Removing a context** (only possible without exported invoices) asks what happens with the customers, templates and layouts scoped to it: make them shared, move to another context, or delete them too.
- **Invoice numbers** may repeat across contexts (`UNIQUE(context_id, number)`). The invoice list shows a muted context line per row, which is enough to tell them apart. Context colors in the list: later.
- **No draft moving** between contexts in v1 (see Later).

## Settings UI

Settings page is grouped: one section per context, titled with the seller name followed by "Layouts" (shared, with the scope option). The context sections are toggleable, only one open at a time. A context section holds its settings, brand color and default layout.

## Settled while discussing

- **Active context** is device state, not data: it lives in `localStorage` like the last route (`src/common/utils/memory.utils.ts`), never in the DB, so it can't end up in a future sync (iCloud or similar). The UI sends it to the few commands that need it (list invoices, create draft, pickers). Drafts and invoices carry their own `context_id`, so the backend keeps no "current context". Missing or archived id falls back to the first non-archived context.
- **No confirm step before export.** The tinted UI makes the active context visible everywhere, preview and context select are enough.
- **Scope is a column, not JSON.** Scope is identity (like `invoices.number` and `status`, which are columns): nullable `context_id` column on `customers`, `templates` and `layouts`, added in migration v5. NULL = shared. The entry JSON stays untouched.
- **Layout "in use" checks** (`layouts_in_use`, `layout_usages` in [store.rs](../../src-tauri/src/store.rs)) scan all invoices regardless of context and stay as they are. "Remove context, delete its layouts" reuses the normal layout-delete usage check.

## Guard: no context or incomplete data

All views except the settings are disabled while the active context is missing or its mandatory data is incomplete (fresh install, an old database whose single settings row is half empty, a newly created context). The app then sends the user to the form of that context.

- **What counts as complete** is decided in Rust, in one place, next to the existing validation gate (the UI only asks, like it does for the export gate in [session.rs](../../src-tauri/src/session.rs)). A context list entry gets `missing: [field, ...]`, empty = complete.
  - **Blocking (guard):** name, address (street, post code, city, country code), VAT id or tax number (one of them, thanks to the prerequisite below), IBAN, a valid number pattern, a valid tax rate or a blank one with a non-empty VAT note.
  - **Soft, flagged "needed for XRechnung", not blocking:** contact name, phone, and the email (also the seller's electronic address). Only the XRechnung profile requires them (the profile is chosen per invoice), the export gate reports them when it matters.
  - **Not required:** BIC, account holder, legal info, intro/closing, payment days (blank = due on issue), layout (defaults to `standard`).
  - **Verified 2026-10-20** by running the real pipeline (own rule check, then Mustang) on the test seller with one field removed at a time, both profiles:
    - Missing name (BR-06), IBAN (BR-50/61) and the VAT-free reason (BR-E-10) are refused in both profiles, by the app's own rule check.
    - Country is refused by `build_invoice`. Street, post code and city are optional in EN 16931 (valid), but XRechnung refuses all three: post code and city by own rules (BR-DE-4/3), an empty street only by Mustang (PEPPOL-EN16931-R008). So all four address fields count as required.
    - Contact name/phone/email: valid in EN 16931, refused in XRechnung (BR-DE-5/6/7, PEPPOL-EN16931-R020).
    - BIC and account holder: valid when missing.
    - **Gap found by the check, decided: (b).** With a tax number but no VAT id both profiles are refused by BR-CO-26 (a seller identifier BT-29/BT-30 or a VAT id BT-31 is needed, a tax number is BT-32 and does not count). Many issuers have only a tax number (small businesses under § 19, a Verein). So the tax number is also written as the seller identifier, BT-29 (plain identifier; BT-30 is meant for registry numbers), and the result is verified with Mustang for both profiles. This is a **separate change that comes before the guard** (the guard then requires "VAT id or tax number"), see [Prerequisite](#prerequisite-tax-number-as-seller-identifier).
- **The mandatory fields must really be required**, in three places that share one rule:
  1. The form marks them as required and shows what is missing. Saving an incomplete context stays allowed (data is filled in steps), the guard keeps the other views disabled until it is complete.
  2. The guard asks the same `missing()` function.
  3. The export gate already refuses most of them with German texts (the rule check in `src-tauri/src/invoice`, before any tool runs), so this is not a new gate: `missing()` is the early, form-time view of the same rules. Keep the two aligned, ideally one list of seller rules both use, so a rule added to the gate cannot be forgotten in the guard. Exported invoices use their frozen settings and are never re-checked against the guard.
- **Routing.** Today `registerRouter` ([routing.utils.ts](../../src/common/utils/routing.utils.ts)) wraps `@lit-labs/router`, routes are lazy via `enter`, and `redirect()` already exists. A route's `enter` can return `false`, so the guard fits there: a shared `guarded(enter)` wrapper for the invoices, customers and templates routes that checks the active context, calls `redirect('/settings/contexts/<id>')` and returns `false` when something is missing. No new router needed.
- Settings routes become `/settings/contexts/:id` (the open context section, matches "one open at a time") and `/settings/layouts`; the plain `/settings` redirects to the active context. These routes are never guarded.
- The restored last route (`restoreRoute()` in [memory.utils.ts](../../src/common/utils/memory.utils.ts)) goes through the same guard, nothing bypasses it.
- Nav links of the guarded views are shown disabled, not hidden, with a short hint ("Angaben zum Kontext fehlen"). The context switch stays usable: switching to another complete context lifts the guard.
- First start with no context at all: the settings route shows the form of a new, empty context ("Neuer Kontext"), saving it creates context 1 and makes it active.
- Only the active context is checked, archived ones are ignored. Once data is complete the guard redirects back to where the user came from, or to the invoice list.
- The guard is a convenience, not the gate: export validation stays as it is, so nothing incomplete can be issued even if the UI is bypassed.

## Prerequisite: tax number as seller identifier

**Done 2026-10-20** (feat commit "a tax number alone identifies the seller"): seller identifier, German finding text and tests are in. Kept here for the reasoning. Independent of contexts, small, done first. **Verified 2026-10-20** with a throwaway change (reverted): when no VAT id is set, the tax number is also added as seller identifier BT-29 (`Identifier::new(tax)`, no scheme) next to the tax registration BT-32. Result with the real pipeline (own rule check, then Mustang incl. the XRechnung rules), tax number only: valid for EN 16931 and XRechnung, with VAT and VAT-free. BT-30 is not needed.
- `seller_party` in [draft.rs](../../src-tauri/src/draft.rs): `if vat_id is blank, push Identifier::new(tax_number) to party.identifiers` (4 lines). With a VAT id nothing changes.
- Tests: tax-number-only seller through the real validator for both profiles (ignored sidecar test like the others), and a unit test that the seller identifier is only set without a VAT id.
- The German finding texts for BR-CO-26/BR-S-02/BR-DE-16 say "USt-IdNr. oder Steuernummer" where they only say USt-IdNr. today (check `src-tauri/src/invoice/german.rs`).

## Data model

Schema is `PRAGMA user_version` based in `src-tauri/src/store.rs` (now 4). Migration **v5**:
- `contexts (id INTEGER PRIMARY KEY, json TEXT NOT NULL, archived_at TEXT)`, `json` = today's `Settings` plus the brand color. No separate name: the seller name is the label everywhere (switch, list line, settings section).
- Move the singleton `settings` row into `contexts` as id 1 (drop the old table, the version check blocks downgrades anyway).
- `invoices`: `ADD COLUMN context_id INTEGER NOT NULL DEFAULT 1`, backfill 1, add `archived_at TEXT`.
- Replace `number TEXT UNIQUE` by `UNIQUE(context_id, number)`. SQLite can't alter a constraint: rebuild the table (create new, copy, drop, rename) inside the migration. There is an existing test that builds a v1 database by hand, add one for v4 → v5 with data.
- `customers`, `templates`, `layouts`: `ADD COLUMN context_id INTEGER` (nullable, NULL = shared). `save_*` write it, loading returns it so the editor can show the scope.

## Backend (src-tauri)

- `Store`: `settings()`/`save_settings()` take a context id. Add `contexts()`, `save_context`, `archive_context`, `remove_context(id, disposition)`. `invoices()`, `next_number`, `finalize`, `save_draft`, `duplicate` take/carry `context_id`. Add `archive_invoice`.
- `customers()`, `templates()`, `custom_layouts()` take the active context and return shared + own (`WHERE context_id IS NULL OR context_id = ?`).
- `Session` in [session.rs](../../src-tauri/src/session.rs) already stores the settings in force; no change besides which settings are loaded.
- `ListEntry` gains `context_id` and `seller_name` (the label shown in the muted list line).
- `commands/settings.command.rs` becomes context commands (list, get, save, create, archive, remove).

## Frontend (src)

- Header: context switch (seller name, tinted by the brand color), "manage contexts" entry.
- Settings view as described above; "new context" starts as a copy of the active one (new color, number counter starts fresh automatically).
- Invoice list: "all contexts" toggle, muted context line per row, archive action for exported invoices.
- Customer/template/layout editors: "only for <context>" option.
- Remove-context dialog with the three choices.
- First run: an existing install has one context and shows no switch (or a quiet one), nothing else changes visually except the blue brand class being set explicitly.

## Tests

- Migration v4 → v5 keeps invoices, numbers, paid/exported state, customers, templates, layouts.
- Same number allowed in two contexts, rejected inside one.
- Counter is per context, including year reset.
- Finalize under context B uses B's seller, an exported invoice is unaffected by later edits to B.
- Scoped customer is hidden from the other context, shared one visible to both.
- Remove context: each disposition (share, move, delete), refused when exported invoices exist.
- Required fields: each blocking field missing alone is reported by `missing()` and refused by the export gate, soft fields are not reported as blocking; empty database and a half-filled settings row both report `missing` fields, a full one reports none.
- Archived context/invoice hidden from the switch/list but still in "all" and in period queries.

## Manual checks

Fresh database (no context): all views disabled, redirected to the new-context form; fill the form, views unlock; old database with missing IBAN: redirected to that context's form; last-route restore with an incomplete context; header switch and UI tint per context, settings sections (one open at a time), invoice list (toggle, context line, archive), new invoice flow under each context, export and preview (seller block, IBAN, footer, layout), pickers with shared and scoped entries, remove-context dialog, first start on a copy of an old database.

## Later

- Context kind (company, Verein, private person) with its own defaults, e.g. VAT handling and legal info fields.
- Move a draft to another context (re-applies seller, layout and number settings, so needs care).
- Archive view for contexts and invoices.
- Context colors in the invoice list.
- Anything that turns out to be shared between contexts in `Settings`.
