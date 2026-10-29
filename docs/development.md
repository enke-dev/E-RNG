# E-RNG – development

macOS desktop app that turns structured invoice data into a **ZUGFeRD / Factur-X PDF** (PDF/A-3 with the
EN 16931 XML embedded) and, if wanted, an **XRechnung** (plain XML). Every file is validated before it can
be saved: a failed check blocks the export.

## How it works

```
form ─► draft ─► invoice model (en16931) ─► CII XML
                         └─► Typst ─► PDF/A-3b ─► Mustang combine ─► ZUGFeRD PDF ─► Mustang validate ─► save
```

- **Rust** owns the data: SQLite storage, the mapping from the form to the EN 16931 model, the rule checks
  (German texts for the findings) and the CII XML (`en16931` / `en16931-formats` crates).
- **Typst** renders the PDF, **Mustang** (with its bundled veraPDF and the official EN 16931 / XRechnung
  rules) attaches the XML and validates PDF/A, XMP and XML. Both run as bundled sidecars, together with a
  minimal Java runtime for Mustang. The app never hands out a file that did not pass.
- **Frontend**: Lit, Web Awesome, signals, German UI. Dark mode follows the system.
- Creating an invoice is one step (`creation.rs`): the rules are checked, the PDF and XML are made and validated, and only if all of it holds the draft gets its number, freezes its settings and layout, locks, and the files are kept in `<data>/invoices/<context>/` with a SHA-256 each (`originals.rs`, `invoices.files`). Nothing is stored when it fails, one creation runs per context at a time. "Exportieren als" (PDF or XML) copies a kept file after checking the hashes, an invoice created before files were kept gets them made again from its frozen data on its first output. The files are never deleted by the app. Created invoices are locked and keep the number, tax rate and seller details they were made with,
  corrections go through "duplicate". A blank tax rate means no VAT is charged: the lines are exempt (E) at 0 % with the reason `vat_note` (default § 19 UStG) as BT-120, and the document shows no rate, no VAT rows and no gross sum, only the reason (`Branding.vat_free`). A blank payment term (`payment_days`) leaves the document without due date and terms; the XML still gets the due date of the issue day, because EN 16931 asks for a due date or terms (BR-CO-25). A seller with only a tax number still meets BR-CO-26 (an identifier or VAT id), as before. German law asks for more than EN 16931 in one place, checked next to the rules (`invoice::recipient_address`, rule `§ 14 UStG`): the customer's address (street, post code, city) is required above 250 € gross and only a warning up to it (Kleinbetragsrechnung, § 33 UStDV); empty address parts are left out of the XML, which forbids empty elements. Until then a draft shows the pattern with the counter masked (`RE-2026-###`) under the title `Rechnungsentwurf`, in the preview and in `Exportieren als PDF` (`invoice_draft_pdf`: the preview as a plain PDF, no XML, no check). The date of service is one day (BT-72) or, with `delivery_end`, a period (BG-14, `Leistungszeitraum`). Numbers are assigned on export only, by a pattern in the settings (`numbering.rs`: date tokens like in layouts, `N` for the counter, `NNN` fills with zeros to three digits, `[text]` stays; the default `[RE-]YYYY-NNN` gives `RE-2026-001`). The pattern only formats. When the counter starts again is a setting of its own (`Settings.number_reset`, `Reset`: `never`, `fiscal_year` (the default), `quarter`, `month`; `Zähler periodisch zurücksetzen` in the settings): the counters run in periods (`Calendar::period`, from the issue date, the reset and the fiscal year start; a quarter is one of the fiscal year, a month a calendar month; the calendar quarters of a short first year are periods of their own, they write the same `GGGG` and `Q` as the fiscal ones that follow, while the short year shares its year period with the fiscal year after it, both write the same `GGGG`), a document keeps its counter and its issue date (`invoices.counter`, `issued_on`, schema 7) when it gets its number, and the next number is the highest counter of the period plus one. A pattern change never confuses the counters; numbers from before (no stored counter) are read back with the current pattern. A reset only works when the pattern tells the periods apart, or numbers would repeat: `numbering::reset_problem` formats three years of days and refuses two days of different periods with the same text (answers are kept per pattern, reset, start and year, it runs on every read of a context) (yearly reset with `NNN`, a fiscal year from April with `YYYY-NNN`; `never` accepts every pattern). It is reported as the field `number_reset` in `Settings::missing`, refused by `context_save` and shown under the pattern (`number_example`). The fiscal year is a context setting (`Settings.fiscal_year_start`, none: the calendar year; a date repeats its day and month every year, before it the calendar year applies): `GGGG`/`GG` write the year it starts in, `gggg`/`gg` the year it ends in, `Q` its quarter, and letters meant as text go in `[brackets]` (schema 7 bracketed `G`, `g` and `Q` of stored patterns, `protect_new_tokens`). To continue the numbers of another program the user sets a counter (`number_sequence_set`, a dialog in the settings under the pattern): it is a floor for the current period of the reset (`Period::key`, which names the reset) in `number_floors` (state, not a setting; one per context, kind and period, removed with its context), `next = max(floor, highest + 1)`, refused at or below the highest number given in the period and dropped when it equals highest + 1. A stored prefix of an older version becomes `[prefix]YYYY-NNN` when settings are read. The seller's legal information (`legal_info`, one free text: what the legal form requires on business letters) goes into the XML (BT-33) and, as `{legal}`, into the layouts (older layouts and exported invoices say `{footer}`, it still works); the footer text of older versions moves there when settings are read (an exported invoice that froze it keeps its footer).

An **offer** (Angebot, `Kind::Offer`) is written like an invoice and numbered on its own. It lives in the `invoices` table (`kind` column, schema 8, numbers unique per context and kind) and has the life cycle of an invoice; its pattern is `Settings.offer.number_pattern` (default `[AN-]YYYY-NNN`) and its reset `Settings.offer.number_reset`, the number floors are per kind (`number_floors.kind`), and its layout and texts follow the invoices' unless `Settings.offer` has its own. Creating one (`creation::make_offer`) builds it with the totals engine and renders the PDF with Typst: no XML, no EN 16931 rule check, no Mustang. The PDF is kept with its SHA-256 under `<data>/offers/<context>/` (`InvoiceFiles.xml` is none); `output` refuses an XML for an offer, and `regenerate`, `set_paid` and `set_files` are for invoices only. An offer has no payment lines, no due date and no date of service, and is valid for a number of days (`InvoiceDraft.validity_days`, from `offer.validity_days` when the draft is made, kept when it is duplicated; blank: no validity). The day itself, the issue date plus the days, is never stored or typed: `InvoiceDraft::valid_until` works it out when the document is shown. It shows as the info row `valid-until` (`Gültig bis`, in the built-in layout after the date) and as the placeholder `{valid_until}` in the letter texts (the default closing text uses it; a text line that names it goes when there is no date) and in the text parts of layouts. Headline, headline of a draft and label of the number are the wording of a kind (`Wording`: `Settings.invoice_wording`, `Settings.offer.wording`, read with `Settings::wording(kind)`; a blank part is the default, so an issuer can write `Offerte`); credit notes keep `Gutschrift`. The settings of a kind are read through `pattern(kind)`, `layout_for(kind)`, `intro(kind)` and `closing(kind)`, a blank draft of a kind comes from `Settings::blank_draft(kind, today)`. Offers open earlier than invoices: `Settings::missing_for_offers()` asks for the seller's name and address and a pattern that parses and a reset it fits (`offer.number_pattern`, `offer.number_reset`; a broken offer pattern or reset does not block invoices). In the UI an offer is the invoice list and editor with `kind="offer"` (`/offers`, `/offers/:id`, "Angebote" first in the menu; `/` stays the invoice list): the routes render a literal each, so Lit makes a separate element per kind, and the texts that differ live in `kind.utils.ts`. The offer list has the stages drafts, valid and expired (a created offer whose valid-until day, `ListEntry.valid_until`, lies before today), no paid badge. The editor leaves out the profile, the service date and the payment card, adds `Gültigkeit (Tage)` (a number prefilled from `offer.validity_days`, the resulting day is its hint) and exports the PDF only; an id of the other kind redirects to its own route. The settings have a page `Angebote` before `Rechnungen`, the counter row and its dialog work per kind. Offers and invoices are linked, many to many (`document_links`, schema 9; `Store::link`, `unlink`, `links`): both ends in one context, the offer and the invoice side checked by the store, links are metadata and can change on drafts and created documents alike, a deleted draft takes its links with it and archiving keeps them, a copy does not inherit them, and an invoice can name the offers it is linked to: `Settings.offer_reference` is the default of new invoices (also the ones `invoice_from_offer` makes), `InvoiceDraft.offer_reference` the choice of one invoice (a switch beside its card of linked offers), and creating the invoice freezes the numbers of its created offers in `frozen_offers` (`Store::offer_numbers`, a draft's preview reads the links as they are). The document gets an info row `offer-reference` (`Zu Angebot AN-2026-003`, several numbers joined by commas, `Branding.offers`, shown by the built-in layout, `InfoField::OfferReference` for own ones); the XML does not carry it (no EN 16931 term fits an offer: BT-14 is the seller's order confirmation, BT-12 a contract, at most one). `Store::invoice_from_offer` (`offer_to_invoice`) makes the invoice draft of a created offer in one transaction: customer, positions, buyer reference and note are copied, layout and texts are the invoice defaults of the context (`Settings::blank_draft(Kind::Invoice, …)`), dated today, linked. The editor of either kind has a card named after what it holds (`Angebot` for one linked offer, else `Angebote`; `Rechnung` / `Rechnungen` on an offer; linked documents open by their row, a remove icon each, a dialog to pick the other kind of the context, the customer's documents first, archived ones included; an unsaved document is saved as a draft first), a created offer has `Rechnung erstellen` (asks first when invoices are linked; disabled while the context lacks what invoices need), and the lists mark a linked document with a link badge (`ListEntry.linked`). A new context starts with the document defaults of the active one (`inheritedDefaults`: tax, payment, patterns, fiscal year, texts, wordings and the offer settings).

**Units** are a list per context (`units` table, schema 10, `units.rs`): a label and a UN/ECE rec. 20 code, seeded with Stk. (C62), Std. (HUR) and pauschal (LS). The settings have a page `Einheiten` in each context's group (`erng-unit-list`): add, change, drag to reorder, delete; every change is saved at once in a dialog. The code is searched, not typed (`erng-unit-code`: code, German or English name; the common codes first, an arrow-key list, a hint that opens the list in the XRepository in the system browser through the opener plugin) in `units::catalog()`, a name for every code `en16931::codes::guard::unit` accepts (2162, `src-tauri/data/unit-names.json`, read on first use and shared with the browser mock; `scripts/unit-names.ts` makes it from the XRepository lists rec20 v3 = Rec. 20 rev. 17 and rec21 v2 = Rec. 21 rev. 11, English names, plus the curated German ones that come first; a test fails when a crate update changes the set, then download again and run the script); `guard::unit` still checks what is saved and names `kwh` as `KWH`; a label is unique per context, case aside, several labels may share a code. A position carries its code and its label (`Position.unit`, `unit_label`): picking a unit writes both, the XML gets only the code and the document prints the label (`Branding.unit_labels`, one per position), so renaming or deleting a unit never changes a created invoice. Older rows have no label: they are read as what the code was printed as before (`units::legacy_label`, a frozen copy of the old table that is never edited, an empty code is C62 "Stk.") and are never rewritten for it. Renaming a unit or changing its code follows silently in the drafts of that context and in the saved positions it sees, shared ones included (a shared position may then show as "nicht in der Liste" in another context, its select keeps the label); created, imported and archived documents are not touched, nor is the time a draft was saved. Deleting a unit that no draft or saved position uses asks once; one that is in use opens a dialog that lists where and asks for the unit that takes its place (same context, `Store::delete_unit`), which is rewritten in the same transaction; the last unit of a context stays. The migration (schema 9 → 10) gives every context the defaults and nothing else; a context that has units already (a database stamped back) is left alone. A new context starts with a copy of the units of the active one (`inheritedUnits`, handed to `context_save`; without it the defaults), removing a context removes its units. Positions and saved positions pick from the units of the document's context (the active one on the Positionen page); a unit that is not in the list stays visible in the select, marked, and can be switched.

A **context** is one issuer (`context.rs`: a company today, maybe a Verein or a private person later): its `Settings` (seller, numbering, defaults) plus a brand color, stored in `contexts` (schema 5; the one settings row of older databases became context 1). An invoice belongs to the context it was made under for good (`invoices.context_id`), numbers are unique per context, and a draft keeps its context when saved again. Customers, saved positions and layouts are shared (`context_id` NULL) unless scoped to one context; the pickers get shared plus the context's own. `Settings::missing()` (and `missing_for_xrechnung()`) say what an issuer still lacks before invoices can be issued, the same things the rule check refuses later; the UI guard (`guard()` in `root.routes.ts`) keeps invoices, customers and templates closed until the active context has none missing, and the offers until it has none missing for offers (`missing_for_offers`). A context with invoices is archived, never removed; one without can be removed, and what is scoped to it is shared, moved or removed with it (`Disposition`). Exported invoices can be archived (`archived_at`), they leave the list and still count for numbers; the status menu (checkboxes: drafts, open, paid, plus archived ones) shows them on request and `Wiederherstellen` puts one back (`invoice_unarchive`). Buttons follow one order: what leaves a view or dialog (Abbrechen, Schließen) is a single button on the left, never in a button group; actions and the main action are on the right (a dialog uses `.dialog-footer` from the base styles, a page its `actions` and `primary` slots, the invoice footer has the leaving button on the left and the actions on the right). Something that is being made and not stored yet (a new invoice or layout) shows as an italic `placeholder` entry at the top of its list instead of a label in the form, and as a `Neu` tag where its status would be (the invoice footer). Leaving a view with unsaved edits is the user's call (`leave.utils.ts`: a view registers a check, the route `enter` hooks in `root.routes.ts` and the context switch ask `confirmLeave`; staying puts the address back; the settings register one, an invoice draft saves itself instead: a stored draft half a second after the last edit, a new one when it is left; quitting right while typing can lose the last half second, macOS ends the app without asking the window). Which context is active is device state (`localStorage`, `context.utils.ts`), not data; its brand is applied as `wa-brand-<name>` on `<html>`, which re-derives every brand token of the UI library.

More: [`.claude/plans/init.md`](../.claude/plans/init.md) (plan, decisions, open items) and
[`.claude/plans/phase1-results.md`](../.claude/plans/phase1-results.md) (what the validators accept and reject).

## Requirements

- macOS on Apple silicon (the pinned sidecars are arm64 only)
- [Bun](https://bun.sh) (version in `.bun-version`), Rust stable, Node (version in `.node-version`)
- No Java needed: the build links its own runtime

## Develop

```sh
bun install
bun run generate     # app icons (from the Icon Composer export src/assets/icon-default.png) and sidecars: downloads Typst, Mustang
                     # and a JDK (checksum-pinned), links the JRE
bun run dev          # tauri dev
```

`bunx vite dev --config .configs/vite.config.ts` runs the UI in a plain browser against a small mock
backend (`src/common/utils/mock.utils.ts`): good for looking at layouts, not for rules or totals.

| Script                           | Does                                                                     |
| -------------------------------- | ------------------------------------------------------------------------ |
| `bun run check`                  | typecheck (`tsc`)                                                        |
| `bun run lint`                   | rustfmt, clippy (`-D warnings`), eslint                                  |
| `bun run format`                 | rustfmt and eslint `--fix` (also runs on commit)                         |
| `bun run test`                   | vitest and `cargo test`                                                  |
| `bun run generate:sidecars:stub` | empty sidecar placeholders, enough for lint and typecheck (what CI does) |
| `bun run build`                  | `tauri build`                                                            |

Tests that run the real tools (Typst, Java, Mustang: golden invoices, negative cases) are ignored by
default:

```sh
bun run generate:sidecars
cd src-tauri && cargo test -- --include-ignored
RENDER_OUT=/some/dir cargo test every_golden -- --include-ignored   # keeps the PDFs to look at
GOLDEN_OUT=/some/dir cargo test                                      # writes the CII XML of the golden set
```

## Layout

```
src/                       frontend
  views/                   invoices and offers (list + detail), invoice-edit, customers, templates, settings (+ root shell and routes)
  common/                  components (one folder each: `<name>/<name>.component.{ts,css}`, styles always in the css file, `ErngElement.withStyles(styles)`), layouts (split, cards, editor: slots only, split and editor have a `wa-split-panel` the user can move (their slots are the panels, so they style their own slots; the divider position is stored under the `position-key` of the view; the list has a minimum and maximum, the detail a minimum; the editor shows only the form where the minimums of both sides do not fit, a container query), views import no styles), api.utils.ts (typed commands), format and form helpers, `reorder.utils.ts` (the drag and arrow-key reorder of the positions and the units: `Reorder` renders the handle, its styles are `reorder.utils.css`), mock backend
src-tauri/src/
  invoice/                 profile choice, rule checks, German finding texts, CII writer, golden test set
  draft.rs                 form draft → invoice model
  units.rs                 units of measure: catalog (data/unit-names.json), defaults, the old labels, checks
  store.rs, model.rs       SQLite and the stored shapes
  render/                  invoice layouts (Typst) and the presentation view of an invoice
  layout_files.rs          own layouts: fonts (family from the file) and pictures (size, pixels)
  layout_bundle.rs         a layout as a file (`.erng-layout`): export, import with its fonts and pictures
  package.rs               Mustang: combine and validate, report parsing
  pipeline.rs             produce and validate the PDF and XML in a work directory
  creation.rs, originals.rs  create an invoice or offer (check, number, freeze, lock, keep the files) and hand the kept originals out
  sidecar.rs               running the bundled tools with a deadline
  commands/                one tauri command per file
scripts/                   sidecars.ts and its pins (sidecars.json), icons.ts
src-tauri/test-fixtures/   a font file for the layout tests (Open Sans, OFL)
```

## Data

The database and the layout files live in the app data directory (`~/Library/Application Support/dev.enke.e-rng`). A
debug build (`tauri dev`) uses `dev.enke.e-rng.dev` next to it, so a development run never migrates the data of an
installed app: an older version refuses a database a newer one has migrated ("written by a newer version"). To start a debug build empty (a fresh setup, someone else's case) without moving that folder away, point it elsewhere: `ERNG_DATA_DIR=/some/dir bun run dev`.

## Messages

`notify(message, variant)` and `notifyError(error)` (`src/common/utils/notify.utils.ts`) show a toast bottom left: the
answer to something the user just did (saved, failed, already there). What stays true while they work, a locked
invoice, what is still missing, an empty list, the failure of an export inside its dialog, is a `wa-callout` on the
page.

## Icons

`<wa-icon name="save">` resolves against the library registered in `src/common/utils/icon.utils.ts`: a few Font Awesome
Free SVGs shipped with the app (the Web Awesome default library would load them from a CDN, which an offline app
cannot). Names say what an icon means (`save`, `trash`), add one by importing its svg there. Buttons with a label have no
icon; only the actions inside a list (remove a position, save it as a template) are icon only, with a `title` and an
`aria-label`.

## Layouts

A layout is a **frame** and **parts**. The frame (`PageSetup.margins`) is where the flowing content (title, intro, the
positions, totals, notices, payment, closing: `render/layouts/standard.typ`) stays; the parts are placed around it by
an anchor (one of the nine points of the page) and an offset in millimetres towards the inside: information block
(`infos`), the address, an image, a text (bold and italic as markdown, `{seller_name}`-like placeholders, a font family),
the page number, a rule. Every page has such a setup; the first page can have one of its own (`first`) with the same
tools. The built-in layout, _Klassisch_, is such a list (`layout::builtin()`, a window letter after DIN 5008 form B), a new
layout starts as a copy of it, so there is no logo setting: a logo is an image part.

Rust builds `PageView` for the template (`render/parts.rs`: text cut into runs, placeholders filled, images copied next to
the template and measured, `data.json`), `prelude.typ` places the parts in the `background` of every page. Typst has one
margin per page style, so when the first page has another frame the content is laid out in two runs (`two-runs`): units
(head, table rows, totals, …) are measured and as many as fit go to the first page, the rest follows on pages with the
other frame.

There is no separate stationery: a letterhead is a picture part as wide as the page (210 mm, at 0 and 0), one per page setup, and pictures always lie behind the other parts. Fonts are a list of uploaded files. Images are PNG, JPG
or SVG up to 5 MB; a raster image with fewer than 150 dpi at its placed size is flagged, the layout sample marks it with a
warning triangle (`Branding.marks`, never in a real invoice).

How numbers, money and dates are written is the layout's `Format` (`render/format.rs`: decimal and thousands separators, currency as symbol or code, before or after, with a space or not, `19 %` or `19%`, the minus sign, IBAN in groups, dates by a dayjs-like pattern with German month names). The default is the German way.

A layout is exported to one JSON file (`layout_bundle.rs`, the form's state, not a stored one) with fonts and pictures as base64 under made-up names, so no path of the user leaks. Fonts go along only if the user says so (they have licences); without them the importer is told which families are missing. Every stored layout has a `uid` (made when it is first stored, kept through renaming, export and import) and an `updated_at` (UTC, set on each save). An import (`layout_import`, `plan` in `layouts.command.rs`) reads the file first and copies nothing until the questions are answered: the same `uid` as a stored layout is the same layout, and replacing it needs a yes (the dialog shows both times and what uses it; the row and its `id` stay, so the default and drafts keep pointing at it); a name another layout has (any case, archived too) is never taken, the user names the import instead (suggested: `Name (Import <date>)`, which does not look like a successor); anything else is stored right away, so it takes an explicit delete to remove it. The file is untrusted, fonts and pictures pass the same intake as picked ones (`store_font`, `store_image`), and a damaged or foreign file is refused as a whole. The built-in layout can be exported too, as a starting point.

`.erng-layout` is a file association of the app (`bundle.fileAssociations`, with an exported type `dev.enke.e-rng.layout` so Finder knows its kind). macOS hands a double-clicked file to the running app, or to a starting one, as `RunEvent::Opened`; `opened-files.command.rs` keeps the paths until the window asks (`opened_files`, also on the `layout-file-opened` event), the root view asks the user to confirm (`opened.utils.ts`), goes to the settings, and the settings import it like a picked file, with the same replace and name questions. It only works in a built app, not in `tauri dev`; Windows and Linux (the path comes as an argument) are not wired.

The sample hands out the box of every part (`<part-box>` metadata, read with `typst query`, `render_sample`), so the editor can tell what is under the pointer: pointing at a part in the form or in the sample marks it orange on both sides, a click in the sample opens it in the form. The settings show the same 30-position sample in every layout (`layout_preview`), with optional guides: red for the
frame, cyan for the aids of a letter (address window, fold and punch marks, middle) and dotted outlines of the parts
(`Branding.guides`). The default layout is a setting; a layout is deleted only while no invoice uses it and it is not the
default, else it is archived. An exported invoice freezes its layout like the settings.
`cargo test -- --include-ignored` validates every golden case in the built-in layout and with letterhead pictures, a font and
image parts.

What is remembered between runs is a convenience of the machine, not a setting: the dividers, the guides toggle and the last open view (`memory.utils.ts`, `localStorage`, wrapped so a missing storage breaks nothing; a blank invoice is not restored) and the size, place and state of the window (`tauri-plugin-window-state`). A divider is stored as the `position` (percent) of its `wa-split-panel`; the limits are its native `--min`/`--max` in the layout's css, which clamp what is drawn, not the stored value.

## Release

Pushing to `main` runs `check`, then bumps the version (conventional commits, `release-it`), builds the app
on macOS into a draft release, commits the bump, publishes the release and writes `latest.json` for the
in-app updater to the `gh-e-rng-latest` branch. Needs the repo secret `TAURI_SIGNING_PRIVATE_KEY`.
Signed ad hoc (`bundle.macOS.signingIdentity` is `-`), not notarized: no Apple Developer ID. A downloaded copy shows
Gatekeeper's "cannot verify" dialog, and System Settings → Privacy & Security offers "Open Anyway". A bundle whose
signature does not verify would say "damaged" instead, with no way around it: check a build with
`codesign --verify --deep --strict E-RNG.app`. Mind that running a release build uses the real data folder, only
`tauri dev` has its own.

## Licences

See [NOTICE.md](../NOTICE.md): the e-invoice model comes from the `en16931` crates (CEN attribution),
Typst, Mustang (with veraPDF) and Temurin are shipped as separate programs.
