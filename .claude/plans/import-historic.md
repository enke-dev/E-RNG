# Importing historic invoices – Plan

Goal: bring invoices written before this app into it, so they count in the Vorgänge, the filters,
the totals and the numbering, with their original file kept and hashed. The app never re-renders,
validates or changes the original.

Status: plan, not started (scope settled 2026-10-29, placed in the Vorgänge 2026-11-03). Related:
the kept originals and units in `docs/development.md`, [steuerberater-export.md](steuerberater-export.md)
(imports belong in the period export), [vorgaenge.md](vorgaenge.md).

## Scope

- **Old, unsigned invoices that went out on paper**: PDF, Word (`.docx`, `.doc`), OpenDocument
  (`.odt`) and Apple Pages (`.pages`).
- **Out**: ZUGFeRD, Factur-X, XRechnung, CII and UBL. No structured reading, no XML import, no
  `en16931-formats` read features. No OCR.
- Credit notes and Stornos are imported too, with negative totals (a `credit` flag, shown as a
  `Gutschrift`/`Storno` tag).

## Model

- **An import becomes a draft** with an `imported` info attached (source file name, its SHA-256, the
  guesses, `credit`). The user edits it like any draft: customer, dates, positions, totals. Typos are
  fixed in the draft, not by importing again.
- **Its Vorgang**: each import gets a Vorgang of its own, named after it (`Rechnung RE-2019-017`, as
  `cases::title_after`); an import can also go into a picked Vorgang of the same customer.
- **The number is taken over as it is and never changes**: entered once (prefilled by a guess),
  stored as `historic_number`, read-only afterwards, unique per context like every number. It counts
  for the next number only if the pattern could have made it (`next_number` as it is).
- **Finishing locks it as historic**: `Als historisch abschließen` instead of `Prüfen und erstellen`.
  Status `historic` (locked like `exported`), its number the `historic_number`, the original kept
  with its hash (`invoices/<ctx>/historic/<safe number>.<ext>`). No new PDF or XML, no validation.
  `Exportieren als` hands out the original. A neutral `Historisch` tag; it counts for totals, filters,
  paid, archive and the Vorgang stage like created invoices.
- **Duplicates are refused**: the same file (hash) imported in any context, a file equal to an
  original E-RNG made, a number taken in the context.

## Flow (it must scale, the volume is unknown)

- `Neu` in the Vorgänge gets `Importieren`: a multi-file picker, then the queue at `/cases/import`.
- All files are read and prefilled in the background; the queue lists them with their guesses
  (number, date, customer, total) and a status.
- The user walks through them with the keyboard: Enter takes the draft and opens the next one, the
  draft form beside the original's preview. Files with complete guesses are taken as drafts in one go
  (`Alle übernehmen`); finishing stays a step per invoice or a multi-select `Als historisch
  abschließen`.
- **Guesses** from the document's text, each marked as a guess until touched: number (near
  "Rechnung", "Rechnungsnummer", "Nr."), invoice date, service date, gross total (near "Gesamt",
  "Summe", "Rechnungsbetrag"), customer name (the address block), positions where a table is
  recognizable. Text from the PDF text layer (`lopdf`), Word and OpenDocument via macOS `textutil`,
  Pages via the `QuickLook/Preview.pdf` in the package. On Windows `.docx` and `.odt` are read as
  zipped XML in Rust, `.doc` and `.pages` come without text. A scan has no text and no guesses.
- **Preview**: PDFs as SVG pages by the linked Typst in `erng-sheet`; other files as their text and
  `Im Standardprogramm öffnen` (an opener scope limited to the queue's files).
- **Customers**: an existing one with the same name is suggested, `Als neuen Kunden speichern`
  creates one; nothing is created automatically.

## Tests

- Store: a historic row reads as historic and is locked (`save_draft`, `delete_draft`, `finalize`
  refuse it); the number unique per context, allowed in another; `next_number` after `RE-2026-017`
  dated 2026 is `RE-2026-018`, after `2026-017` it stays `RE-2026-001`; paid, archive and the stage
  of its Vorgang work; a context with imports is archived, not removed; the hash lookup finds imports
  and created originals.
- Guesses: number, dates, total and customer from text fixtures (a German invoice, a credit note,
  one without a number).
- Originals: kept with the hash, handed out only when the hash still matches, importing the same
  file again refused.
- Ignored real-tool tests: PDF pages to SVG; `textutil` text of a docx fixture.

## Manual checks

- Import a PDF, a Word file and a Pages file at once; walk the queue by keyboard; `Alle übernehmen`.
- A number that is taken, one that does not fit the pattern; the next number in a new draft.
- In the Vorgänge: the tag, filters, the stage, totals; open an import: original preview, paid,
  archive, `Exportieren als`.
- The same file twice; an E-RNG original.

## Later

- Correcting the typed data of a historic invoice while the original stays fixed.
- Import from a folder.
- OCR for scans (Vision framework).
- Imports in the Steuerberater export: the originals and their VAT rows.
