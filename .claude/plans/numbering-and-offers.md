# Number sequences and offers: plan

## Review decisions (2026-10-29) – they override the text below

- **The pattern only formats, the reset is a setting (second review, 2026-10-29).** Counters no longer restart because some text around them changes (that implicit rule is gone, and with it everything below that says so). Per kind a setting says when the counter starts again: `Settings.number_reset` for invoices, `Settings.offer.number_reset` for offers, `never | fiscal_year | quarter | month` (default `fiscal_year`; the select is called `Zähler periodisch zurücksetzen` with the options `Nie`, `Je Geschäftsjahr (Kalenderjahr)` (or `Je Geschäftsjahr (ab 1. April)`, naming the start of the fiscal year), `Je Quartal` and `Je Monat`; a quarter is one of the fiscal year, a month a calendar month). The counters run in periods (`Calendar::period(reset, date)`: from the issue date of a document, the reset and the fiscal year start). A document keeps its counter and its issue date (`invoices.counter`, `issued_on`) when it gets its number, so a changed pattern never confuses the counters; the next number is the highest counter of the period plus one, or the floor of the period when that is higher. A floor belongs to the period of the reset it was set under (`Period::key()` names the reset). Numbers without a stored counter (older data) are read back with the current pattern. A reset needs the period in the number, or numbers would repeat: settings that do not (yearly reset with `NNN`, a fiscal year from April with `YYYY-NNN`) are refused with a German message (`numbering::reset_problem`, found by formatting three years of days: two days of different periods that make the same text), and `never` accepts every pattern. A short first fiscal year counts with the one after it (its period key is the `GGGG` the text shows).
- **Fiscal year is a setting of the context**: a switch "Geschäftsjahr entspricht Kalenderjahr" (on by default). Off, the user sets the start of the first business year as a date (`fiscal_year_start: Option<Date>`): its day and month repeat every year, before it the calendar year applies (a short first year, Rumpfgeschäftsjahr, for a business that switched). Tokens: `GGGG`/`GG` the year the fiscal year starts in, `gggg`/`gg` the year it ends in (§ 4a Abs. 2 Nr. 2 EStG counts a fiscal year in the year it ends; reports write `2026/27`, i.e. `GGGG/gg`). `Q` stays the quarter of the fiscal year. Upper and lower `g` are free in `date_pattern` (it knows `D`, `M`, `Y` only); the v7 pattern migration brackets unbracketed `G`, `g` and `Q`.
- **Offer reference on the invoice: later.** Then a per-invoice choice with a default in the context settings, frozen when the invoice is created. For now the link stays internal (list badge, the card of linked documents). Open question 5 is closed.
- **"Rechnung erstellen" only from created offers** (open question 6 closed as proposed).
- **Offers have no service date**, the field is hidden (open question 4 closed as proposed).
- **Number label typed per kind** (`Wording.number_label`, open question 2 closed as proposed).
- **Validity, both ways, both in the default layout**: an info row "Gültig bis" (`valid-until`) and a placeholder `{valid_until}` for the texts (closing text of offers, text parts of layouts). The built-in layout shows the info row and the default offer closing text carries a sentence with the placeholder; no fixed sentence in the template (replaces the "validity sentence" decision; open question 3 closed).
- **Offers open earlier than invoices**: the offers route has its own guard, `Settings::missing_for_offers()`: seller name and address only (an offer needs no IBAN, tax id or contact). Invoices keep `missing()`. The guard in `root.routes.ts` picks by route (open question 8 closed).
- Open question 7 (equal patterns): allowed, numbers are unique per kind, as proposed.

Goal: invoice numbers can continue from an older system and can restart by fiscal year. The same machinery numbers a second kind of document, the **offer** (Angebot). An offer is written like an invoice, has its own consecutive numbers and a validity date, and gets a PDF only. "Rechnung erstellen" turns an offer into an invoice draft and remembers that the two belong together.

Status: planned 2026-10-28, not started. Assumes the parallel work has landed: `invoice_unarchive` with the "Archiviert" filter, duplicating drafts, the draft PDF without ZUGFeRD (headline "Rechnungsentwurf", masked number `RE-2026-###`), service period as a date or a range, and autosave of drafts. Related: [steuerberater-export.md](steuerberater-export.md), which must leave offers out (see Backend).

## How numbering works today (for reference)

- `numbering.rs`: `Pattern::parse` splits the pattern into the text before the counter, the counter's width and the text after it. `Pattern::next(date, existing)` renders the before and after text for the date, keeps the existing numbers that have exactly that text around their digits, and returns the highest plus one. So the counter restarts whenever the text around it changes (`YYYY` gives a yearly restart, `YYYYMM` a monthly one). It *continues* when that text comes back: `MM-NNN` in March 2027 goes on from March 2026. Numbers are unique by construction, and `UNIQUE(context_id, number)` backs that up.
- `Store::next_number(context_id, pattern, date)` reads every number of the context. `creation::next_number` and `preview_invoice` call it with the issue date of the draft.
- `Settings.number_pattern` is one per context. `number_example(pattern)` shows the first number of today in the settings.

## Decisions

### Number sequences

- **One sequence per context and kind** (`Kind::Invoice`, `Kind::Offer`). Each kind has its own pattern. Numbers are unique per (context, kind). An offer and an invoice may in theory carry the same text, but the defaults never do (`[RE-]…` and `[AN-]…`).
- **Start number = a floor per period.** "Period" is the stretch of days one counter runs through, set by the reset (see the second review decision), so a floor fits without new rules:
  - `next = max(floor of this period, highest issued in this period + 1)`.
  - A floor only applies in the period it was set for. With `[RE-]YYYY-NNN`, setting 88 in 2026 still lets 2027 start at 001. Without a reset the period never changes, so the floor holds for good. Changing the pattern does not change the period, so the floor stays.
  - Setting a counter at or below the highest number already issued in that period is refused, with the number named: "RE-2026-087 ist bereits vergeben, der Zähler kann nur auf 88 oder höher gesetzt werden." This refusal is about intent: `max` would make a lower floor harmless anyway, but it would silently do nothing.
  - A floor can be lowered again as long as nothing at or above the new value was issued. One floor per period, replaced, not accumulated. Setting it to exactly highest + 1 removes it.
  - Considered and rejected: storing the floor as a virtual "last issued number" (`RE-2026-087`) and mixing it into the existing numbers. It works the same way, but it cannot be lowered and reads like a real invoice.
- **The floor is state, not a setting.** It gets its own table, not a field in `Settings`, because:
  1. A new context starts from the active one's numbering defaults and would copy the floor.
  2. Settings are frozen into every created document.
  3. The settings form saves the whole struct, so a stale form would write an old floor back.
  4. The refusal rule needs the database.

  It is changed in its own step (a dialog with a warning), saved at once.
- **Fiscal year.** `Settings.fiscal_year_start` (month 1–12, default 1) is a fact of the context, shared by both kinds. Two new tokens, for number patterns only (not for layout date formats):
  - `GGGG` / `GG`: the fiscal year, named by the calendar year it starts in. With April as the start, April 2026 to March 2027 is `2026`.
  - `Q`: the quarter of the fiscal year (the calendar quarter when the fiscal year starts in January).

  `[RE-]GGGG-NNN` with start month 4 gives `RE-2026-…` on 2027-03-31 and `RE-2027-001` on 2027-04-01. With a yearly reset the period changes with the fiscal year, so the counter restarts then.
- **Uniqueness (§ 14 UStG) stays enforced by the design.** A counter only restarts by the reset, and settings whose pattern does not tell the periods apart are refused, so no setting can make two equal numbers in one kind.
- **Existing patterns keep their output.** `Q`, and `G` in runs of two or more, become tokens. Migration v7 wraps every unbracketed `G` and `Q` of a stored pattern in brackets (`RG-YYYY-NNN` → `R[G]-YYYY-NNN`), so nothing that is already written changes. The help text says that letters meant as text belong in brackets, as it already does for `N`.

### Offers

- **Same table as invoices, with a `kind` column.** An offer's body is exactly an `InvoiceDraft` (customer, dates, positions, texts, layout, frozen settings and layout), and its life cycle is the invoice's: draft → created (numbered, frozen, locked, original kept) → output, archive, restore, duplicate. With a shared table:
  - Save, autosave, duplicate, delete, archive, unarchive, finalize and the creation mutex work unchanged.
  - Cross-cutting scans are right without extra code. `layout_usages` and `layouts_in_use` also see offers, so a layout an offer uses cannot be deleted. `context_usage` and `remove_context` also see offers, so a context with created offers is archived, not removed.
  - Turning an offer into an invoice copies a row within one table.

  The cost:
  - Kind-specific queries must filter: the list, `next_number`, `set_paid`, `set_files`/`regenerate`, and the planned Steuerberater export.
  - The `UNIQUE` constraint needs a table rebuild, which has a precedent in `migrate_to_contexts`.

  A separate `offers` table would copy about ten store functions and their tests, and every cross-cutting scan would need a `UNION`. `kind` is a column, not JSON, because it is identity (the same reasoning as `context_id` in multi-context.md). The table and the Rust types keep their names (`invoices`, `StoredInvoice`, `InvoiceDraft`). Renaming would touch every SQL string for nothing; the docs say what they hold.
- **No ZUGFeRD and no XRechnung.** Creating an offer checks the input (`build_invoice`, which is also the totals engine), renders the PDF with Typst (`render_pdf`, PDF/A-3b without an attachment) and keeps it with its SHA-256. There is no EN 16931 rule check and no Mustang run. The live check for offers shows input problems and totals only, no findings. The profile select is hidden.
- **Validity.**
  - `InvoiceDraft.validity_days: Option<u32>` (serde default). Settings have `offer.validity_days: Option<u32>`, default 30; blank means no validity. The day itself is never stored or typed: it is the issue date plus the days, worked out when the document is shown (`InvoiceDraft::valid_until`) and for the list.
  - A new offer starts with the days of the settings; the editor has a number input, no date input.
  - Duplicating an offer dates it today and keeps the days.
  - The document shows it twice, like the due date of an invoice: as an info row `valid-until` ("Gültig bis") and as the sentence "Dieses Angebot gilt bis 30. November 2026." where an invoice has its payment lines.
  - Offers have no payment lines, no due date and no service date (the field is hidden; internally the delivery date is the issue date).
- **Document wording per kind**, a `Wording { title, draft_title, number_label }` in the settings:
  - Invoice defaults: `Rechnung`, `Rechnungsentwurf`, `Rechnungsnummer`.
  - Offer defaults: `Angebot`, `Angebotsentwurf`, `Angebotsnummer`.
  - The user can write `Offerte` / `Offertenentwurf` / `Offertennummer`. The labels are typed out because German compounds cannot be derived reliably.
  - Credit notes keep `Gutschrift`.
  - The draft PDF of the parallel work uses `draft_title` instead of its fixed "Rechnungsentwurf".
- **Settings per kind stay where invoices have them.** The invoice fields stay flat in `Settings`, because frozen settings of created invoices are read without `normalized()` (see `Branding::for_draft` reading `footer`), and moving the fields would need custom deserializing. Additions:
  - `invoice_wording: Wording`.
  - `offer: OfferSettings { number_pattern, validity_days, intro, closing, layout: Option<String> (None = the invoice default layout), wording }`.
  - Accessors make the code kind-agnostic: `Settings::pattern(kind)`, `layout_for(kind)`, `intro(kind)`, `closing(kind)`, `wording(kind)`.
- **Offer → invoice ("Rechnung erstellen")**:
  - Only from a *created* offer: the link should point at a numbered document the customer has seen.
  - It makes a new invoice draft in the same context and opens it. The draft copies customer, positions, buyer reference and note. It takes the *invoice* defaults for layout and texts: an offer's intro ("gerne bieten wir an …") does not belong on an invoice. It is dated today, with no frozen data and no `valid_until`. The link is stored in the same transaction.
  - It can be done again: partial or Abschlag invoices each get a draft with all positions, and the user removes what is not billed. If invoices are already linked, it asks first ("Zu diesem Angebot gibt es schon RE-2026-004. Noch eine Rechnung anlegen, z. B. eine Teilrechnung?").
- **Relation: many-to-many, as metadata.** One offer can lead to several invoices (Teil-, Abschlags-, Schlussrechnung), and one invoice can settle several offers (main offer plus Nachtragsangebot).
  - A link table, both ends in the same context, one end of each kind.
  - Links are not part of the locked document. They can be added or removed on drafts and on created documents, by picking.
  - Deleting a draft removes its links. Archiving keeps them.
  - In v1 the links are not printed on the invoice (see Open questions).
- **Navigation:** "Angebote" is the *first* menu entry: `/offers`, `/offers/:id`, guarded like invoices. `/` stays the invoice list, so remembered addresses and the start view do not change. The settings pages follow the menu order: Unternehmen, Bankverbindung, Angebote, Rechnungen.

## Data model

Migration **v7** (`SCHEMA_VERSION` is 6 today; if the parallel work lands a migration first, take the next free number). It runs in one transaction, guarded like the others so a database stamped back by hand migrates again:

1. **Rebuild `invoices`** (only if it has no `kind` column): create `invoices_new` with all current columns plus `kind TEXT NOT NULL DEFAULT 'invoice'` and `UNIQUE (context_id, kind, number)`. Copy with explicit column names (`id, context_id, number, status, draft, total, exported_at, paid_at, updated_at, archived_at, files`), drop, rename.
2. **`document_links`**:
   ```sql
   CREATE TABLE document_links (
       offer_id INTEGER NOT NULL,
       invoice_id INTEGER NOT NULL,
       created_at TEXT NOT NULL,
       PRIMARY KEY (offer_id, invoice_id)
   );
   CREATE INDEX document_links_invoice ON document_links (invoice_id);
   ```
   Foreign keys are not enabled in this app (`PRAGMA foreign_keys` is never set), so the store checks the kinds and the shared context, and deletes links with a draft.
3. **`number_floors`**:
   ```sql
   CREATE TABLE number_floors (
       context_id INTEGER NOT NULL,
       kind TEXT NOT NULL,
       period TEXT NOT NULL,
       next INTEGER NOT NULL,
       set_at TEXT NOT NULL,
       PRIMARY KEY (context_id, kind, period)
   );
   ```
   `period` is `Period::key()`: the reset and the period, like `year:2026`, `quarter:2026-3`, `month:2026-07`, `never`.
4. **Patterns:** read every `contexts.json`, bracket the unbracketed `G` and `Q` in `settings.number_pattern` (a helper in `numbering.rs`, `protect_new_tokens`), write it back.

Without a migration (serde defaults):
- `Settings.number_reset = fiscal_year` (existing contexts get the default, even where the stored pattern cannot tell the periods apart: the settings then ask for a pattern or reset that does), `Settings.fiscal_year_start = 1`, `invoice_wording`, `offer` (pattern `[AN-]YYYY-NNN`, 30 days, wording defaults).
- `InvoiceDraft.validity_days = None`.
- `InvoiceFiles.xml` becomes `Option<StoredFile>` (`#[serde(default)]`; every stored invoice has `Some`).
- Offer originals go to `<data>/offers/<context_id>/<safe number>.pdf`, kept apart from `invoices/`, so equal names of the two kinds cannot overwrite each other.

## Backend (src-tauri)

**`model.rs`**
- `enum Kind { Invoice, Offer }`, serde kebab-case (`invoice`, `offer`). This is not `DocumentKind`, which en16931 already uses in `render/view.rs`.
- `Wording`, `OfferSettings`, `Settings.fiscal_year_start`, `Settings.invoice_wording`, `Settings.offer`, and the accessors listed in Decisions.
- `Settings::blank_draft(kind, today) -> InvoiceDraft`: layout, intro, closing and `validity_days` from the settings of that kind. This moves the defaults that `invoice-edit.view.ts` fills in today into one place, which the conversion uses too.
- `StoredInvoice.kind`.
- `ListEntry` gains `kind`, `valid_until` and `linked: u32`.
- `LayoutUsage` gains `kind`, so the settings can say "Angebot AN-…".
- `InvoiceFiles.xml: Option<StoredFile>`.

**`numbering.rs`**
- `pub struct Calendar { pub fiscal_year_start: u8 }` with `fn fiscal(date) -> (year, quarter)`.
- A token pass for `GGGG`, `GG` and `Q` outside brackets turns them into bracketed literals, then `date_pattern` runs as before. `counters()` is unchanged.
- `Calendar::period(reset, date) -> Period` with `Period::key()`.
- `Pattern::counter_of(number, date, &Calendar) -> Option<u32>`: the strip and parse, only for numbers without a stored counter.
- `Pattern::next(&self, date, &Calendar, existing, floor: Option<u32>) -> String` computes `max(floor, highest + 1)`.
- `Pattern::masked(date, &Calendar)`: the masked draft number. Generalize the parallel work's version if it has one.
- `protect_new_tokens(pattern) -> String` for the migration.

**`store.rs`**
- `migrate`: v7 as above.
- `save_draft(id, context_id, kind, draft, total)`: `kind` is used for new rows only. The update path keeps the stored kind.
- `invoices(context, kind, archived)` filters by kind and fills `linked` with a subquery count.
- `next_number(context_id, kind, &Settings, date)` reads the numbers of that kind and the floor of the period.
- `sequence(context_id, kind, &Settings, date) -> Sequence { next_number, counter, highest: Option<String>, floor: Option<u32> }`.
- `set_counter(context_id, kind, &Settings, date, next: u32) -> Result<Sequence>`. It holds one connection lock for the read and the write. It refuses `next <= highest`, deletes the floor when `next == highest + 1`, and upserts it otherwise.
- `finalize`: the unique violation message names the kind.
- `set_paid` and `set_files` add `AND kind = 'invoice'`.
- `duplicate(id, today)`: the copy keeps the validity days. Links are not copied.
- `delete_draft` also deletes the draft's `document_links` rows.
- `invoice_from_offer(offer_id, today, &Settings) -> StoredInvoice` in one transaction: the offer must be created, then insert the draft (from `blank_draft(Kind::Invoice, …)` plus the copied fields) and the link.
- `links(id) -> Vec<Linked { id, kind, number, status, customer_name, issue_date }>`.
- `link(offer_id, invoice_id)` (checks kinds and context, idempotent) and `unlink(offer_id, invoice_id)`.
- `context_usage` gains `offers: usize`. `drafts` and `exported` count invoices only. `remove_context` refuses while any of the three is non-zero.

**`creation.rs`**
- `next_number` and `create` read `stored.kind` and use `settings.pattern(kind)` and the calendar.
- For `Kind::Offer`, `create` uses `make_offer`:
  1. `build_invoice` (input problems become `ProduceError::Input`).
  2. `render::render_pdf` into a `Work` directory.
  3. `originals::store_pdf`.
  4. `finalize`.

  Same `Creating` guard, same "nothing stored on failure".
- `output` refuses an XML target for an offer. `regenerate` is for invoices only (offers always have their files).

**`originals.rs`**
- `folder(kind, context_id)`.
- `store_pdf(data, kind, context_id, number, pdf)` next to the existing `store`.
- `discard` and `verified` handle `xml: None`.

**`render/view.rs`**
- `Branding::for_draft` becomes `Branding::for_document(kind, draft, settings, customs)`. It picks layout and texts by kind and adds:
  - `title`, `number_label` (from `wording(kind)`, or `draft_title` for the draft PDF)
  - `offer: bool`
  - `valid_until: Option<Date>`
- `build`:
  - `title` comes from the branding (the credit-note case stays).
  - `info_rows`: the `number` label comes from the branding. For offers there is no `due-date` and no `service-date`, and there is a `valid-until` row.
  - `payment_lines` is replaced by the validity sentence for offers.
- If the parallel draft-PDF work put a `draft` flag or title override into `Branding`, fold it into `title`.

**`layout.rs`**
- `InfoField::ValidUntil` (key `valid-until`), added to `builtin()` after `Date`. Rows that a document lacks are left out already.
- In `parts.rs`, `info()` maps the new key.

**`context.rs`**
- `Settings::missing` also reports `offer.number_pattern` when it does not parse.

**Commands** (the `invoice_*` commands act on both kinds; only those named here change)
- `invoices.command.rs`:
  - `invoices_list(context_id, kind, archived)`.
  - `invoice_blank(context_id, kind)` uses `blank_draft`.
  - `invoice_save(id, context_id, kind, draft)`.
  - `invoice_duplicate` keeps the validity days of an offer.
  - New: `offer_to_invoice(offer_id) -> StoredInvoice`, `document_links(id)`, `document_link(offer_id, invoice_id)`, `document_unlink(offer_id, invoice_id)`.
- `check-invoice.command.rs`: `check_invoice(context_id, kind, draft)` returns no findings for offers.
- `preview-invoice.command.rs`: `preview_invoice(draft, id, context_id, kind)`, using the kind-aware `next_number` and `for_document`.
- `invoice-files.command.rs`: `invoice_next_number` uses the stored kind. `invoice_create` and `invoice_output` are unchanged in signature.
- `contexts.command.rs`:
  - `number_example(pattern, fiscal_year_start)`.
  - New: `number_sequence(context_id, kind) -> Sequence` and `number_sequence_set(context_id, kind, next) -> Sequence`. Both use today and the *saved* settings.
- `lib.rs`: register the new commands.

**Steuerberater export (planned):** its queries must use `kind = 'invoice'`. Add a note to its plan.

## Frontend (src)

**`api.utils.ts`, `mock.utils.ts`**
- `type Kind = 'invoice' | 'offer'`.
- The new fields, the changed signatures, the new commands. The mock keeps a `kind` on its rows and a links array.

**`common/utils/kind.utils.ts`** (new): the UI texts per kind in one place.
- Invoice: route `/invoices`, `Rechnungen`, `Neue Rechnung`, `Rechnungen suchen`, `Noch keine Rechnungen.`, create button `Prüfen und erstellen`, confirm `Rechnung … erstellen? Danach ist sie gesperrt.`
- Offer: `/offers`, `Angebote`, `Neues Angebot`, `Angebot erstellen`, `Angebot … erstellen? Danach ist es gesperrt.`
- Document titles come from the settings wording, not from here.

**`views/root.routes.ts`**
- `/offers` and `/offers/:id`, entered with `entering(() => import('./invoices/invoices.view.js'))` and guarded.
- `offers(selected)` is its own `html` literal, so Lit makes a separate element and does not just flip `kind` on the invoice list.

**`views/root.view.ts`**
- `NAV` starts with `['/offers', 'Angebote']`. `NEEDS_CONTEXT` gets `/offers`. `isActive` already works through `startsWith`.

**`common/utils/memory.utils.ts`**
- `restorable` also accepts `/offers/<id>` and refuses `/offers/new`.

**`views/invoices/invoices.view.ts`** gets `@property() kind: Kind = 'invoice'`.
- Texts come from `kind.utils.ts`, and `api.invoicesList(context, kind, archived)`.
- Status filter for offers: Alle, Entwürfe, Erstellt, Gültig, Abgelaufen (created, `valid_until` before today), plus the parallel "Archiviert".
- Offers show no paid badge. They show a muted `Abgelaufen` tag, and a badge when invoices are linked (`linked > 0`; new icon in `icon.utils.ts`, e.g. `file-invoice`).
- `goto` paths use the kind's route.

**`views/invoice-edit/invoice-edit.view.ts`** gets `@property() kind`. It is passed by the list, and for stored documents it is checked against `stored.kind`: an id of the other kind redirects to its own route.
- The blank draft comes from `api.invoiceBlank(contextId, kind)` (the inline defaults go away).
- Offers:
  - Hidden: profile select, service date and range (parallel work), payment control.
  - Shown: a "Gültig bis" date with a hint showing the days, moving with the issue date as described in Decisions.
- Draft footer (offer): the parallel draft actions (draft PDF, duplicate, delete), then brand `Angebot erstellen`.
- Created footer (offer):
  - Middle: the number tag and a `Gültig bis <date>` tag (warning variant when expired).
  - Right: `Duplizieren`, `Archivieren` / `Wiederherstellen`, a plain `Als PDF exportieren` (no split button, one format), brand `Rechnung erstellen`.
  - `Rechnung erstellen` asks first when invoices are already linked, calls `offerToInvoice`, toasts "Rechnungsentwurf aus Angebot AN-2026-003 angelegt.", and goes to `/invoices/<id>`. It announces `invoice-changed` so the offer list updates its badge.
- Card of the linked documents (both kinds, drafts and created), titled `Angebot` / `Angebote` on an invoice and `Rechnung` / `Rechnungen` on an offer (singular for exactly one):
  - The linked documents as list items (number or `Entwurf`, customer, date) that open them.
  - A remove icon per item (icon only, with `title` and `aria-label`, as other list actions).
  - `Angebot verknüpfen…` / `Rechnung verknüpfen…` opens a dialog listing the other kind in the same context: same customer first, then a search, archived ones included.
  - New documents (no id yet) save first.
- Export file name `<number>.pdf`.

**`views/settings/settings.view.ts`**
- `PAGES` and `CONTEXT_PAGE` get `offers` (`Angebote`) before `invoices`.
- Company page: `Geschäftsjahr beginnt im` (month select, Januar default; hint "Nur für GGGG, GG und Q in den Nummernmustern").
- Invoices page: `Überschrift`, `Überschrift im Entwurf`, `Bezeichnung der Nummer`, and the counter row under the pattern.
- Offers page:
  - `Muster der Angebotsnummer` plus the counter row
  - `Gültigkeit (Tage)` (blank: no validity)
  - the three wording fields
  - `Einleitung`, `Schlusstext`
  - `Standard-Layout` (first option `Wie Rechnungen` = `null`)
- Counter row:
  - It shows "Nächste Nummer: RE-2026-088", with "(fortgesetzt ab 88)" when a floor applies.
  - A `Zähler ändern…` button opens a dialog with a number input and a warning callout: "Nur ändern, um die Nummern eines anderen Programms fortzusetzen. Eine Nummer darf nie doppelt vergeben werden: Werte bis zur höchsten vergebenen Nummer (RE-2026-087) werden abgelehnt." Buttons are `Abbrechen` and `Ändern`. It saves at once with `numberSequenceSet`, and a refusal shows in the dialog.
  - The button is disabled while the pattern or the fiscal month has unsaved edits ("Erst speichern"), because the period depends on the saved reset and fiscal year, the next number on the saved pattern.
- Help text for patterns: add `GGGG`/`GG` (Geschäftsjahr), `Q` (Quartal), "Buchstaben als Text in [eckige Klammern]", and "ohne Jahr zählt ein Monats- oder Quartalszähler im nächsten Jahr weiter".
- `FIELD_LABELS` gets `offer.number_pattern`.
- The remove-context dialog wording mentions offers (from `ContextUsage.offers`).

**`docs/development.md`**
- Numbering paragraph: floors, fiscal tokens, sequences per kind.
- The context paragraph: offers and links.
- The layout list: `valid-until`.
- The data folder: `offers/`.

## Tests

**Rust, `numbering.rs`**
- Fiscal year with start 4: `GGGG` on 2027-03-31 is 2026 and on 2027-04-01 is 2027. `GG` works. `Q` is fiscal (2026-04 → 1, 2026-03 → 4). With start 1, `Q` is the calendar quarter.
- `[RE-]GGGG-NNN` continues over 1 January and restarts on 1 April.
- The floor: next = max(floor, highest + 1); it is ignored in another period; it is kept when the pattern changes.
- `masked`.
- `protect_new_tokens` keeps the output of old patterns identical (`RG-YYYY-NNN`, `Q-NNN` rendered before and after).

**Rust, `store.rs`**
- v6 → v7 migration with data: kind `invoice`; numbers, files, archived and paid kept; patterns bracketed.
- The same number text is allowed once per kind and refused twice within a kind.
- `next_number` per kind.
- `set_counter`: refuses at or below the highest, replaces, can lower, removes at highest + 1, applies to drafts' previews.
- Lists by kind.
- `set_paid` refused on offers.
- Links: n:m; refused across contexts or between equal kinds; removed with a deleted draft; kept on archive.
- `invoice_from_offer`: refused for draft offers; copies customer and positions; takes the invoice layout and texts; links.
- `layout_usages` sees offers.
- A context with created offers cannot be removed.

**Rust, `render/view.rs`**
- Title and number label from the wording (`Offerte`).
- An offer has no payment lines, no due date and no service date, and has the validity row and sentence.
- An invoice is unchanged.

**Rust, `model.rs`**
- Old settings and drafts deserialize with the new defaults.
- `blank_draft` for both kinds.

**Rust, `creation.rs`** (ignored, needs Typst only)
- Creating an offer keeps one PDF under `offers/<ctx>/`, has no XML, gets `AN-2026-001`, and freezes and locks.
- An offer with a missing price stores nothing.
- Output of an offer as XML is refused.

## Manual checks

- **Settings:** set the invoice counter to 88 (the warning shows, then save). The next invoice is `RE-2026-088`. Try 50 after issuing 88 (refused, with the number named). Lower an unused floor.
- **Fiscal year:** change the fiscal month to April with `[RE-]GGGG-NNN` and look at the example. Leave a pattern change unsaved: the counter button is disabled.
- **Offer settings:** change the headline to `Offerte` and see it in the preview, the draft PDF and the created PDF.
- **Offers:**
  - The menu entry is first.
  - New offer: "Gültig bis" follows the issue date. Autosave works. The draft PDF shows `Angebotsentwurf` and `AN-2026-###`.
  - Create (confirm), export the PDF.
  - Duplicate (today, new validity), archive, filter "Archiviert", restore.
  - An expired offer shows the tag.
- **Offer → invoice:** "Rechnung erstellen" opens an invoice draft with the invoice texts, and both show the link. Do it a second time (the question appears). Link a second offer manually to that invoice, unlink it. Delete the invoice draft: the link is gone.
- **Two contexts:** separate offer counters and folders. An old database migrates and its invoices look unchanged.

## Sequencing

1. Numbering alone (floors, fiscal tokens, settings counter row): useful without offers, and it ships first.
2. `kind` column, offer backend, rendering.
3. Offer frontend.
4. Links and "Rechnung erstellen".

Each step passes `bun run lint` and `bun run test` on its own.

## Later

- Offer status "Angenommen" / "Abgelehnt" (dates, list filters). An offer revision chain ("ersetzt AN-2026-003").
- Billed versus offered amount per offer. Proper Abschlags- and Schlussrechnungen (deducting earlier partial invoices, § 14 (5) UStG).
- Printing the offer reference on the invoice (an info field `offer`, frozen at creation).
- Other kinds on the same sequences: Auftragsbestätigung, Lieferschein, Gutschrift as its own sequence.
- Setting a counter for a future period (e.g. in December for next year).
- Renaming the table and types to `documents` / `StoredDocument` once a third kind exists.

## Open questions

1. **Fiscal year name:** start year (`2026` for 04/2026–03/2027, proposed), end year, or `2026/27`? Is a token for the second year needed?
2. **Number label:** keep the typed-out `number_label` per kind, or use one neutral label ("Nummer") for both and save two fields?
3. **Validity sentence:** fixed text (proposed) or editable, e.g. a `{valid_until}` placeholder in the offer's closing text?
4. **Service date on offers:** hidden (proposed), or an optional "Ausführungszeitraum"?
5. **Offer reference on the invoice:** should "Zu Angebot AN-2026-003" be printed on the invoice now (then frozen at creation) rather than later?
6. **Draft offers:** may "Rechnung erstellen" also start from a draft offer, or only from a created one (proposed)?
7. **Equal patterns:** offers and invoices may share a number text (unique per kind, proposed). Should the settings warn or refuse when both patterns are identical?
8. **Context guard:** offers stay behind the same guard as invoices (IBAN etc. required). Should offers open earlier, with only name, address and tax id?
