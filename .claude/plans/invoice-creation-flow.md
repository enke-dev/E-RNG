# Invoice creation flow – Plan

Goal: make the end of an invoice's life cycle explicit. Today "Erstellen und prüfen" does many things at once and the lock happens as a side effect of picking file names. Instead: **Prüfen und erstellen** turns a draft into a real invoice (validate, number, freeze, lock, store the files), **Exportieren als** hands the stored files out.

Status: implemented 2026-10-23 (backend with tests incl. the real tools, frontend checked by type check and lint, not yet clicked through in the app). Related: [steuerberater-export.md](steuerberater-export.md) needs the stored files as well.

## How it works today (for reference)

1. `Neue Rechnung` is a form, nothing stored. `Entwurf speichern` stores the draft (no number, editable), also implicitly when another invoice opens or "Schließen" is pressed.
2. `Erstellen und prüfen` (locked: `Erneut speichern`): saves the draft again, `produce_invoice` builds the invoice with the *next free number as a tentative one*, runs the rule check, renders the PDF/A-3 with the embedded XML, validates it with Mustang. The files stay in a temp work folder, held by a session (`session.rs`) that gates exporting.
3. A dialog shows the result and PDF/XML checkboxes. `Speichern unter…` opens the file pickers, and only then `export_invoice` copies the files and `finalize` assigns the number, freezes settings and layout, sets `exported_at` and locks. Closing anything discards the temp files.

Problems: lock, number and file output are one step triggered by file names; the PDF carries a tentative number that finalize may refuse ("already taken"); the app never keeps the files, "Erneut speichern" regenerates them.

## Decisions

- **Two steps.** `Prüfen und erstellen` (irreversible) and `Exportieren als` (repeatable).
- **Confirm before creating**: a small dialog "Rechnung RE-2026-004 erstellen? Danach ist sie gesperrt." (`ask`, buttons "Erstellen" and "Abbrechen"), showing the number the invoice will get.
- **The originals are stored on creation**: PDF and XML are written into the app's data folder in the same step, with a SHA-256 each. Outputs copy these files, nothing is regenerated, so what was sent is what is archived (and what the Steuerberater export hands out).
- **Failure keeps the draft**: if the rule check or Mustang refuses, nothing is stored: no number, no lock, no files. The problems show in the invoice view, above the preview where the live findings are now, not in a modal.
- **Locked invoice**: `Erneut speichern` goes away, `Exportieren als` is all it has.
- **Live check stays as it is** (totals and rule findings while editing); PDF/A and Mustang validation only run on creation, they are slow.
- **Status display**: in the list a created invoice carries a blue check badge (created and validated; no red state, a failed validation never produces an invoice), next to the existing green check badge for paid. The detail footer shows the status centered between the leaving button on the left and the actions on the right (a three-column footer: `minmax(0, 1fr) auto minmax(0, 1fr)`, so it stays centered when a side is empty): the `Entwurf` tag for a draft, the number tag for a created invoice, plus a `Bezahlt am <date>` tag when paid. No check in the footer, the number says the same. A new, unsaved invoice shows no status, the placeholder entry in the list says it is new.
- **Exportieren als** is a split button that reads "Exportieren als | PDF": the left part exports, the right part is a dropdown that holds the format and switches between `PDF` (default) and `XML` (default for an XRechnung). One format per export, PDF and XML together are not needed. No dialog, there is nothing left to show. Later items: `Per E-Mail…`, `Drucken`. File names default to `<number>.pdf` / `.xml`.

## Data model

Migration **v6** on `invoices`:
- `files TEXT` (JSON): `{ "pdf": {"path", "sha256"}, "xml": {"path", "sha256"} }`, paths relative to the data folder, NULL for drafts and for exported invoices that predate this.
- Folder `<data>/invoices/<context_id>/<safe number>.pdf|.xml` (`safe_name` in `pipeline.rs` already makes harmless names). Never deleted by the app: archiving an invoice keeps its files, removing a context needs no invoices.

Invoices exported before this change have no files. The first `Exportieren als` regenerates them from the frozen settings and layout, validates, stores them (and marks them as regenerated in the JSON), then hands them out as everyone else. Do not backfill in the migration: it needs the sidecars.

## Backend (src-tauri)

- **`invoice_create(id)`** replaces `produce_invoice` and the finalize half of `export_invoice`:
  1. A mutex per context serializes creations (a single-user app, but it removes the number race).
  2. Take the next number of the context (`next_number`), build the invoice with it, rule check, render, Mustang.
  3. On success in one go: write the files, `finalize` (number, frozen settings and layout, `exported_at`, status, `files`), return the stored invoice. On any failure: delete what was written, return `ProduceError` as now, change nothing.
- **`invoice_next_number(id)`** for the confirm dialog (the preview already knows how to ask for it).
- **`invoice_output(id, pdf_target, xml_target)`**: checks the SHA-256 of each stored file, copies it, removes the copies already written if a later one fails. Refuses drafts. A file whose hash differs is an error ("Die gespeicherte Datei wurde verändert"), never silently fixed.
- **Legacy**: `invoice_output` on an invoice without `files` calls the same producing code with the frozen data (no new number) and stores the result first.
- Remove: `produce_invoice`, `export_invoice`, `discard_work_dir`, `session.rs` and its gate (the gate becomes "files are only stored after validation"), `Sessions` state in `lib.rs`. `layout_preview` and the other previews do not use sessions.
- `context.rs`/`store.rs`: `StoredInvoice` gains the files info (paths are not secrets, but the UI only needs "has files" and "regenerated").

## Frontend (src)

- `invoice-edit.view.ts`: drop the `Export` state machine and the dialog (`renderReady`, `renderFailure`, `closeExport`, `saveFiles`).
  - Draft footer: `Abbrechen`/`Schließen` left; right: `Entwurf löschen`, `Entwurf speichern`, brand button `Prüfen und erstellen` (disabled while the live check has blocking problems, as now).
  - Click: save the draft, ask for the number (`invoice_next_number`), confirm, `invoice_create` with the button in loading state. Success: toast "Rechnung <number> erstellt", the view switches to the locked state. Failure: callout in the view with the same rendering as the old failure dialog (`problems`, `findings`, Mustang issues).
  - Locked footer: `Schließen` left; right: `Duplizieren`, `Archivieren`, `Exportieren als` (split button).
- Status in the middle of the footer: the tag and `Bezahlt am`; the blue check badge is in the list (`invoices.view.ts`); the existing paid control keeps working. The footer grid goes back to three columns (it was reduced to two when its label was dropped).
- `api.utils.ts`, `mock.utils.ts`: the new commands, the old ones removed.

## Tests

- Rust (sidecar tests ignored by default like the others): creating a valid draft stores two files, a number, frozen settings, `exported_at`, status; an invalid one stores nothing and the next creation still gets the same number; two creations in one context get different numbers; output copies byte-identical files; a changed stored file is refused; output of a legacy invoice regenerates once and later outputs reuse the stored files; migration v5 → v6 with data.
- Store unit tests for `files` round trip and for archived invoices keeping them.

## Manual checks

Draft: `Prüfen und erstellen` with the confirm dialog (cancel keeps the draft), with a failing rule (callout in the view, still editable), with a valid one (locked, number, check tag). Locked: `Exportieren als` for PDF and XML, file picker cancelled, an existing file overwritten. Open an old exported invoice and `Exportieren als` (regeneration). Mark paid and see the tag and the list check. Two contexts: numbers per context, files in separate folders.

## Later

- `Per E-Mail…`, `Drucken`, "Im Finder zeigen" after output.
- Re-verify stored files against their hashes in a maintenance view; backup/sync of the invoices folder together with the database.
- Include the stored files in the Steuerberater export (see its plan).
