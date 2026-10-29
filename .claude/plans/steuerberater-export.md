# Steuerberater export and DATEV – Plan

Goal: hand the tax advisor everything for a period (month, quarter, year) in one step, later optionally as a DATEV import file.

Status: idea, not started. Builds on contexts (the context filter and per-context settings, see [docs/development.md](../../docs/development.md)). Tier A alone also works for a single context, so it can start earlier if the order changes.

## Tiers

### A. Period export (do first)

- Filter: date range with presets (last month, last quarter, last year, custom), status (exported / paid / unpaid), context.
- Which date counts is a per-context setting:
  - Soll-Versteuerung: `issue_date`
  - Ist-Versteuerung: `paid_at`
  Both fields exist (`StoredInvoice`).
- Output: one ZIP with
  - the ZUGFeRD PDFs (XML embedded; DATEV reads ZUGFeRD directly) and optionally the XRechnung XMLs,
  - `uebersicht.csv` (and maybe `.xlsx`): number, issue date, delivery date, customer, net, VAT per rate, gross, paid_at, status,
  - a summary with totals per tax rate (helps the Umsatzsteuer-Voranmeldung).
- Delivery: save dialog, then reveal in Finder. No mail integration.
- GoBD: only exported (immutable) invoices are included; drafts never.
- Offers (`kind = 'offer'`, same table) are left out: every query of the export filters `kind = 'invoice'`, also the counts and totals per tax rate.
- The files come from the invoices' stored originals (PDF and XML written on creation, with their hashes), see [docs/development.md](../../docs/development.md). Invoices created before originals were kept have none until their first output regenerates them, the export must do the same for those or list them as "without files".

### B. DATEV Buchungsstapel CSV (only if the advisor uses it)

EXTF format, one booking per outgoing invoice. Needs per-context settings:
- SKR03 or SKR04, Sachkontenlänge, Wirtschaftsjahr start
- Berater- and Mandantennummer
- revenue account per tax rate (incl. VAT-free cases)
- debtor account: collective account or one Personenkonto per customer
- BU-Schlüssel per tax case

Fields: Umsatz, Soll/Haben, Konto/Gegenkonto, BU-Schlüssel, Belegdatum, Belegfeld1 = invoice number, Buchungstext = customer name.
Payments are out of scope by default (the advisor books bank statements); optional later.
Strict format with many quirks: build against a real sample import from the advisor, don't rely on the spec alone.

### C. DATEV API / document ZIP (not planned)

API needs Marktplatz partner registration and OAuth, too heavy for this app. The DATEV document-import ZIP format is unverified, look it up properly before promising it.

## Open questions

1. Does the advisor use DATEV (Unternehmen online / Belegtransfer)? What do they want now: PDFs, list, Buchungsstapel?
2. Soll- or Ist-Versteuerung, per context?
3. Credit notes and cancellations: not in the UI yet, but the export must include them once they exist (sign handling in CSV/EXTF).
4. Incoming invoices: out of scope, or a simple receipts folder added to the ZIP?
5. CSV flavour for the overview: German Excel (`;`, decimal comma, UTF-8 BOM) as default?

## Backend (tier A)

- New `src-tauri/src/export_period.rs` (or `commands/export-period.command.rs`): query invoices by context/date field/status, collect files, write ZIP + CSV. Pure function for "rows from invoices" so it is unit-testable without files.
- Store: `invoices_between(context, field, from, to, filter)` returning full `StoredInvoice` (not only `ListEntry`).
- Number/amount formatting: reuse `render/format` helpers, amounts as strings like the rest of the model (no floats).
- Setting on context: `tax_basis: soll | ist`.

## Frontend (tier A)

- "Export for advisor" entry in the invoice list (or its own view): period presets, status, preview count and totals before exporting, then the save dialog.
- Invoice list: same period filter to reuse (the filter is useful on its own).

## Tests

- Rows for a period under Soll vs. Ist (an invoice issued in March, paid in April lands in different periods).
- Boundaries: first/last day of the period, year change.
- Drafts never exported, unpaid excluded when "paid only".
- CSV escaping (`;`, quotes, umlauts), totals per rate equal the sum of the rows.
- ZIP contains every referenced file, names unique and sortable (number-based).

## Manual checks

Invoice list filters, export dialog and presets, opened ZIP and CSV in Excel/Numbers (German locale), a PDF from the ZIP opened in the DATEV/ZUGFeRD viewer the advisor uses.

## Leftovers

- Tier B settings UI (accounts) and EXTF writer, after a sample from the advisor.
- Payment bookings, incoming invoices.
