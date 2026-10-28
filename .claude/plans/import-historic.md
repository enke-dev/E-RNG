# Importing historic invoices – Plan

## Review decisions (2026-10-29) – a revised scope, it overrides the text below

The import is for **old, unsigned invoices that went out on paper**: PDF, Word (`.docx`, `.doc`), OpenDocument (`.odt`) and Apple Pages (`.pages`). **ZUGFeRD, Factur-X, XRechnung, CII and UBL are explicitly out**: no structured reading, no `en16931-formats` features, no XML import. Everything about structured sources below is dropped.

- **An import becomes a draft** (a normal `draft` with an `imported` info attached: source file, its hash, the guesses) that the user edits like any draft: customer, dates, positions, totals. Typos are fixed in the draft, not by removing and importing again.
- **The number is taken over as it is and never changes**: entered once in the import (prefilled by a guess), stored on the draft as `historic_number`, read-only afterwards. No pattern has to match and no suggestion is made; it is unique per context like every number. It counts for the next number only if the pattern could have made it (the existing `next_number` behaviour, no extra rule).
- **Finishing locks it as historic**: "Als historisch abschließen" instead of "Prüfen und erstellen". It gets status `historic` (locked like `exported`), its number is the `historic_number`, the original file is kept with its SHA-256 as the original (`invoices/<ctx>/historic/<safe number>.<ext>`). No new PDF or XML, no validation (it went out on paper). "Exportieren als" hands out the original file. The list shows a neutral `Historisch` tag; it counts for totals, filters, paid and archive like created invoices.
- **Credit notes and Storno**: imported too, with negative totals (a `credit` flag on the import info, shown as a `Gutschrift`/`Storno` tag).
- **Fast batch flow** (the volume is unknown, so it must scale): "Importieren" takes many files at once; all of them are read and prefilled in the background, then a queue view lists them with their guesses (number, date, customer, total) and a status; the user walks through them with the keyboard (Enter accepts the draft, the next one opens), corrects what is wrong in the draft form beside the original's preview. Files whose guesses are complete can be taken as drafts in one go ("Alle übernehmen"); finishing them stays a step per invoice or a multi-select "Als historisch abschließen".
- **Prefill guesses** from the document's text: number (near "Rechnung"/"Rechnungsnummer"/"Nr."), invoice date, service date, total (near "Gesamt"/"Summe"/"Rechnungsbetrag", gross), customer name (the address block), positions where a table is recognizable; each guess is marked as such in the form until the user touches it. Text comes from the PDF text layer (`lopdf`), Word and OpenDocument via macOS `textutil` (`.doc`, `.docx`, `.odt`), Pages via the `QuickLook/Preview.pdf` inside the package (then like a PDF). A scan without text gets no guesses. On Windows (see build-targets.md) `textutil` is missing: there `.docx`/`.odt` are read as zipped XML in Rust, `.doc` and `.pages` are offered without text.
- **Customers**: an existing customer with the same name is suggested, "Als neuen Kunden speichern" creates one; nothing is created automatically.
- Open questions 1–8 are closed by the above (seller mismatch and currency checks were about structured sources and are gone).

Goal: bring invoices that were written before this app (Word, other tools, ZUGFeRD from elsewhere) into it, so they appear in the list, the filters and the totals. Their numbers count for numbering and stay unique per context, and their original file is archived with a hash. The app never re-renders, never validates and never changes them.

Status: plan, not started. Related:
- [invoice-creation-flow.md](invoice-creation-flow.md): originals, hashes, `invoice_output`.
- [units.md](units.md): imported positions carry codes, and labels come from the unit list.
- [steuerberater-export.md](steuerberater-export.md): imports should be in the period export.

## Recommendation: imported, locked, not a draft

Import into a status of its own, **`imported`**, locked from the first moment. Not a draft:
- **The number was already issued and sent.** A draft has no number (`number` is NULL), so it would not reserve the number and would not count for `next_number`. A draft also offers "Prüfen und erstellen", which would give it a *new* number and a *new* document.
- **The document already exists outside the app.** What counts is the file the customer got. The app keeps that file as the archived original (SHA-256, like `originals.rs`) and hands out that file only. It never regenerates it, and it never runs Mustang or the rule check on it (the app did not make it, and old invoices may legally lack things the checks now require).
- **`exported` would be wrong:** it means "made and validated by E-RNG with stored PDF and XML". The blue check badge says "Erstellt und geprüft", which an import is not.
- **Locked follows the existing rule** that issued invoices do not change. Mistakes are fixed by removing the import and importing again (open question 1).

## Decisions

- **Sources:**
  - ZUGFeRD/Factur-X PDF, and plain XRechnung/CII/UBL XML: the exact data is read from the XML.
  - Plain PDF, `.docx` and `.doc`: entered by hand next to the original.
  - Anything else is refused. No OCR.
- **Parsing:** `en16931_formats::read` (pure Rust) with features `["cii", "ubl", "zugferd"]`. `zugferd` adds `lopdf` (and `flate2`, already in the lockfile), and `ubl` is needed because many XRechnungs are UBL. No Mustang needed. Its CLI `--action extract` would only be the fallback if the dependency graph is unwelcome.
- **Text as a helper only:**
  - Word: `/usr/bin/textutil -convert txt -stdout` (built into macOS, offline, handles `.doc` and `.docx`), run through `sidecar::run` with a 20 s timeout.
  - PDF: `lopdf` text extraction, best effort.
  - The text appears in a panel to copy from. Field suggestions come later.
- **Preview of the original:**
  - PDFs are rendered to SVG pages by the bundled Typst (0.15 places PDF pages as images) and shown in the existing `erng-sheet`.
  - Word files show the extracted text and a button "Im Standardprogramm öffnen".
- **Numbers:**
  - Any non-blank number is allowed. It is unique per context (the existing `UNIQUE (context_id, number)`), and the same number in another context is fine.
  - It counts for `next_number` automatically when the pattern could have made it (`RE-2026-17` counts for `[RE-]YYYY-NNN` in 2026).
  - When it does not fit, the wizard says so and offers a pattern that would fit (`numbering::suggest`, e.g. `2026-017` → `YYYY-NNN`). It saves the pattern only after an `ask`. A number only counts through the pattern; there is no hidden counter offset (open question 2).
- **Data entry:**
  - Required: number, invoice date, customer, gross total.
  - Optional: service date, net and VAT (one row per rate), paid on, a note, positions.
  - For structured sources, number, dates, positions and totals come from the XML and are read-only. Only the customer link, "bezahlt am" and the note are entered.
- **Customer:** a copy in the draft, as for every invoice. The wizard suggests an existing customer (same VAT id, else same name) and offers "Als neuen Kunden speichern".
- **Context:** the active one. A ZUGFeRD seller that differs from the context's seller (VAT id or name) gives a warning. A document whose *buyer* is the context's seller is an incoming invoice and is refused.
- **Duplicates are refused:**
  - The same file (SHA-256) already imported in any context: "Bereits importiert als RE-… (Kontext …)".
  - A file that equals an original E-RNG created.
  - A number taken in the context.
- **Wizard:** an "Importieren" button in the invoice list, then a multi-file picker, then the route `/invoices/import`. Each file gets one page, in the same split as the invoice view: form left, original right. The list shows the italic placeholder "Import 2 von 5". Footer: "Abbrechen" left; right: "Überspringen" and brand "Importieren". After the last file it opens the last imported invoice.
- **The invoice view of an import is read-only:**
  - A callout: "Importiert am … aus ‹Datei›. Nicht mit E-RNG erstellt oder geprüft."
  - A summary card, the read-only positions if any, and the payment control.
  - The original as the preview.
  - Footer: "Schließen" left; centre: number tag plus `Importiert` tag (plus `Bezahlt am`); right: "Duplizieren", "Archivieren", "Import entfernen" (danger), and brand "Original exportieren".
- **Removing an import** frees its number. The original file stays in the data folder ("the files are never deleted by the app"), and importing it again reuses the file.
- **Totals:** `invoices.total` holds the gross total (BT-112 for structured sources, typed for others), like every invoice. The net, VAT per rate and amount due are kept in the import info for the Steuerberater export.

## Data model

Migration **v8** (v7 if it lands before units):

```sql
ALTER TABLE invoices ADD COLUMN imported TEXT; -- JSON, see ImportInfo; NULL for everything else
```

Guard it with `has_column` like v6. Rows get `status = 'imported'`, `number` set, `exported_at` NULL, `files` NULL, `paid_at` optional, `updated_at` = time of import.

`model.rs`:

```rust
pub enum Status { Draft, Exported, Imported }          // "imported"

#[serde(rename_all = "kebab-case")]
pub enum ImportSource { Zugferd, Xml, Pdf, Word }

pub struct VatRow { pub category: String, pub rate: Option<String>, pub net: String, pub vat: String }

pub struct ImportedTotals {
    pub currency: String,          // "EUR"
    pub net: Option<String>,
    pub vat: Option<String>,
    pub gross: String,
    pub due: Option<String>,       // BT-115 when it differs (prepaid)
    pub vat_rows: Vec<VatRow>,
}

pub struct ImportInfo {
    pub source: ImportSource,
    pub original: StoredFile,      // invoices/<ctx>/imported/<safe number>.<ext>, sha256
    pub file_name: String,         // as picked
    pub imported_at: String,
    pub totals: ImportedTotals,
    pub seller_name: Option<String>, // as the document names it
    pub profile: Option<String>,   // BT-24 of a structured source
    pub credit_note: bool,         // type code 381
}

pub struct StoredInvoice { /* … */ pub imported: Option<ImportInfo> }
```

`ListEntry` needs no new field (`status` tells imports apart). The draft of an import is a normal `InvoiceDraft`:
- customer, issue and delivery date (`delivery_end` from the XML period), and positions (may be empty).
- `profile` is `XRechnung` when BT-24 says so.
- `buyer_reference` and `note` are set.
- `layout`, `intro`, `closing`, `frozen` and `frozen_layout` are None.

## Backend (src-tauri)

- `Cargo.toml`: `en16931-formats` features `["cii", "ubl", "zugferd"]`. Add `lopdf = { version = "0.44", default-features = false }` (the version the crate pins) for page count and text. Check during implementation that text extraction does not need a non-default feature.
- **`store.rs`. This part needs care:**
  - `stored()` maps `"imported"` → `Status::Imported` explicitly. Today anything other than `"exported"` becomes `Draft`, which would make an import editable.
  - The SELECTs in `invoice()` and `invoices()` read the `imported` column.
  - `set_paid`, `archive_invoice` and `unarchive_invoice` accept `status IN ('exported', 'imported')`.
  - `save_draft`, `delete_draft`, `finalize` and `set_files` stay draft- or exported-only, which is correct.
  - `context_usage` gets `imported: usize`. `remove_context` counts it, so a context with imports is archived, not removed.
  - `invoices()` shows `imported.seller_name` before the context's seller.
  - New: `import_invoice(context_id, number, draft, total, info, paid_at) -> StoredInvoice`. It maps the unique violation to "Die Nummer … ist in diesem Kontext schon vergeben."
  - New: `remove_import(id)` (only `status = 'imported'`), `number_taken(context_id, number) -> bool`, and `find_by_hash(sha256) -> Option<(id, context_id, number)>`. `find_by_hash` checks `json_extract(imported, '$.original.sha256')` and the `files` hashes of created invoices.
- `numbering.rs`:
  - `Pattern::counter(&self, date, number) -> Option<u32>`, taken out of `next` (which then uses it).
  - `pub fn suggest(number: &str, date: Date) -> Option<String>`: the counter is the last digit run that is not the year; YYYY or YY where they match the date; all other text goes in `[brackets]`; the result must `Pattern::parse` and `counter` must accept the number again.
- New `src-tauri/src/import.rs`:
  - `detect(bytes) -> ImportSource`: `%PDF`; `PK\x03\x04` with `word/document.xml`; `D0 CF 11 E0`; `<`. The size limit is 30 MB. The file is untrusted and is read once into memory.
  - `read(path) -> Candidate { source, sha256, file_name, fields: Option<ImportFields>, warnings: Vec<String>, text: Option<String> }`. For PDF and XML it runs `en16931_formats::read`. A PDF without an embedded invoice (`zugferd::Error::NoInvoice`) becomes `Pdf` with text. Read entries become warnings, never refusals.
  - `fields_from(invoice: &en16931::Invoice) -> ImportFields`: buyer → `Customer`; lines → `Position` (name and description, quantity, `unit_code`, `unit_label` from the unit list or `units::catalog_name` or the code, net price); BG-22 → totals; BG-23 → `vat_rows`.
  - `text_of(path, source) -> Option<String>`: textutil or lopdf, capped at 50 000 characters.
  - `warnings_for(context, candidate)`: a seller mismatch, a document addressed to the context's seller, a currency other than EUR, a credit note.
- `originals.rs`:
  - `keep_original(data, context_id, number, from: &Path, ext) -> StoredFile`: target `invoices/<ctx>/imported/<safe_name(number)>.<ext>`. An existing file with the same hash is reused; one with a different hash gets a `-2` suffix.
  - `verified` becomes `pub(crate)` so the output and the preview can check the hash.
- `render/mod.rs`: `render_original(sidecars, dir, pdf: &Path, pages: usize) -> Result<Vec<String>>`. It writes a `main.typ` that places `image("original.pdf", page: p)` for each page on pages sized to the image (`width: auto, height: auto, margin: 0pt`), then reuses `compile(…, "page-{0p}.svg")` and `read_pages`. The page count comes from `lopdf`. A PDF Typst cannot read gives an error the view shows as "Vorschau nicht verfügbar".
- New `commands/import.command.rs`:
  - `invoice_import_read(path, context_id) -> Candidate` (async, `spawn_blocking`). Nothing is stored.
  - `invoice_import_pages(path) -> Vec<String>` for the wizard, and `invoice_original_pages(id) -> Vec<String>` for a stored import (hash-verified first).
  - `invoice_import_number(context_id, number, issue_date) -> NumberCheck { taken, counts, next, suggestion }`.
  - `invoice_import(context_id, path, sha256, fields: ImportFields) -> StoredInvoice`, in this order:
    1. Re-read the file and refuse it if the hash differs from the candidate.
    2. For structured sources, take number, dates, positions and totals from the file again rather than from the UI.
    3. Check the fields: number not blank, dates parse, gross is a decimal.
    4. `keep_original`, then `import_invoice` (and the customer when `save_customer`). If storing fails, the copied file is removed again.
  - `invoice_import_remove(id)`.
  - `invoice_output_original(id, target)`: imported only, `verified`, then copy.
  - Register all of them in `lib.rs`.
- `preview_invoice` and `check_invoice` refuse an `id` whose status is `imported`, as a safety net. `creation::output` already refuses anything that is not `Exported`.
- `capabilities/default.json`: `opener:allow-open-path` limited to `$APPDATA/invoices/**` for "Im Standardprogramm öffnen". The wizard opens the picked path through a command that checks it is in the current import queue. Never a broad `**` scope.

## Frontend (src)

- `api.utils.ts`:
  - `Status` adds `'imported'`, and `StoredInvoice.imported: ImportInfo | null`.
  - The types `ImportSource`, `ImportInfo`, `ImportedTotals`, `VatRow`, `ImportFields`, `Candidate`, `NumberCheck`, and `ContextUsage.imported`.
  - The calls `invoiceImportRead`, `invoiceImportPages`, `invoiceOriginalPages`, `invoiceImportNumber`, `invoiceImport`, `invoiceImportRemove`, `invoiceOutputOriginal`.
- New `src/common/utils/import.utils.ts`: an `importQueue` signal of picked paths with their candidates. After a reload the queue is empty and the wizard offers "Dateien wählen".
- `invoices.view.ts`:
  - Outlined "Importieren" in the filter bar's `actions` before "Neue Rechnung". It opens the plugin-dialog `open({ multiple: true, filters: [{ name: 'Rechnungen', extensions: ['pdf', 'xml', 'docx', 'doc'] }] })`, fills the queue and goes to `/invoices/import`.
  - `current === 'import'` renders `<erng-invoice-import slot="detail">` and the placeholder entry.
  - The status filter gets `imported` ("Importiert"). `open` and `paid` include imports. `exported` stays E-RNG-created only.
  - Imports get a neutral badge with a new `import` icon (`@fortawesome/fontawesome-free/svgs/solid/file-import.svg` in `icon.utils.ts`), titled "Importiert, nicht mit E-RNG erstellt".
- New `src/views/invoice-import/invoice-import.view.ts` (+ `.css`), inside `erng-editor`:
  - A form card "Dokument" shows the source, the file name and the warnings (`wa-callout`).
  - A card "Rechnung": number with a live `invoiceImportNumber` check ("vergeben", or "zählt nicht für die nächste Nummer, die nächste wäre …" with "Muster „…“ übernehmen", which runs `ask` and then `contextSave`); invoice and service date; customer select plus "Aus dem Dokument" / "Neuer Kunde".
  - A card "Beträge": gross required; net and VAT rows optional, with a hint when net + VAT ≠ gross. Read-only for structured sources.
  - Optional read-only positions, plus "Bezahlt am" and "Notiz".
  - A collapsible "Text aus dem Dokument" for Pdf and Word.
  - The preview slot holds `erng-sheet` with the pages, or the text and "Im Standardprogramm öffnen" for Word.
  - It registers a leave check while a page has typed data.
- `invoice-edit.view.ts`:
  - `locked` becomes `status !== 'draft'`.
  - For `imported`, skip `runCheck` and `renderPreview`, load `invoiceOriginalPages`, render `renderImported()` instead of the form cards, and show the footer described under Decisions.
  - `renderStatus` gets the `Importiert` tag.
- `settings.view.ts`: the remove-context dialog counts imports.
- `mock.utils.ts`: one imported invoice and the new commands (reading returns a fixed candidate).

## Tests

- Rust (`store.rs`):
  - Migration v7 → v8 keeps every row.
  - An `imported` row reads as `Imported`, and `save_draft` and `delete_draft` refuse it.
  - The number is unique per context but allowed in another context.
  - `next_number` after importing `RE-2026-017` (dated 2026) is `RE-2026-018`; after `2026-017` it stays `RE-2026-001`.
  - `set_paid`, archive and unarchive work on imports; `finalize` and `set_files` refuse them.
  - `remove_import` frees the number.
  - A context with only imports cannot be removed.
  - `find_by_hash` finds imports and created originals.
- Rust (`numbering.rs`):
  - `counter`: padded, unpadded, and not matching.
  - `suggest`: `2026-017` → `YYYY-NNN`; `RE-2026-17` → `[RE-]YYYY[-]NN` (or the equivalent `[RE-]YYYY-NN`); `R17/26` → `[R]NN[/]YY`. Each suggestion parses and counts its own number.
- Rust (`import.rs`):
  - The CII the app writes for a golden invoice (`en16931_formats::write_unchecked(…, Syntax::Cii)`) is read back into fields with the same number, dates, buyer, positions and totals. No sidecars needed.
  - A UBL sample.
  - A fixture ZUGFeRD PDF made once by the app (`src-tauri/tests/fixtures/import/zugferd.pdf`).
  - A plain PDF without XML becomes `Pdf`.
  - Magic detection for docx, doc and garbage; a file over the limit is refused.
- Ignored tests with real tools: `render_original` gives one SVG per page; on macOS, `textutil` text of a docx fixture contains its invoice number.
- `originals.rs`: an import is kept with its hash, `invoice_output_original` refuses a changed file, and importing again reuses the file.

## Manual checks

- Import three files at once: a ZUGFeRD PDF from another tool (fields read-only, preview of the PDF), a Word file (text panel, typed amounts), and a plain PDF.
- Skip one. Enter a number that is taken, and one that does not fit the pattern: take the suggested pattern and see the next number in a new draft.
- In the list: badge, filter "Importiert", year and customer filters, totals.
- Open an import: no form, original preview, "Bezahlt am", "Original exportieren", archive and unarchive, "Import entfernen", import again.
- A file imported twice is refused. An E-RNG-created PDF is refused.
- Remove a context that has imports: it is archived instead.

## Later

- Field suggestions from the extracted text (regexes for "Rechnungsnummer", dates, "Gesamtbetrag").
- Imports in the Steuerberater export: originals and the `vat_rows`.
- Correcting the typed data of an import while the original stays fixed.
- Import from a folder.
- OCR for scans (Vision framework, not cheap).
- Sorting the list by invoice date, so a large batch of imports does not push recent work down.

## Open questions

1. Should a wrong import be removable (proposed, the file stays), or should its typed data be correctable instead or as well?
2. When historic numbers do not fit the pattern, is adopting a matching pattern enough, or do you want a "counter starts at" setting per context?
3. Should historic credit notes and cancellations (Storno) be imported? With a negative total, and how should they show in the list?
4. Any invoices in a currency other than EUR?
5. Should a ZUGFeRD seller that does not match the active context be refused, or should the wizard offer to pick the context per file?
6. Roughly how many documents is a first import: dozens, or hundreds (then a faster mode with fewer fields)?
7. Should imports create customer records automatically, or only when asked (proposed)?
8. Is `.doc` really needed, and anything else (`.pages`, `.odt`, scanned JPG)?
