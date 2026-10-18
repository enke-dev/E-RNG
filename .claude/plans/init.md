# E-Invoice macOS App – Plan

Goal: Desktop app (Tauri) that generates a **ZUGFeRD/Factur-X PDF** (PDF/A-3 + embedded CII XML) and optionally an **XRechnung** (plain XML) from structured invoice data.

## Status and leftovers

State on 2026-10-17: phases 1-5 are done (spike, model and XML, Typst template, validation gate, UI), plus
a polish round and the app icon. The first push (`chore: init`) ran the whole release pipeline green and
published **v0.0.1**: draft published, tag `v0.0.1`, release commit `chore: release 0.0.1` on `main`,
`gh-e-rng-latest` created with a valid `latest.json`, assets `E-Rng_0.0.1_aarch64.dmg` (116 MB) and the
signed updater archive (`.app.tar.gz` + `.sig`). The repo is public. The icon commit then released **v0.1.0**
(minor: a `feat` commit; the first release with a changelog entry, the run took 5 minutes with the sidecar
cache). Work after that (list and detail, templates, sortable positions) is committed and ships with the next release. 48 Rust tests (a handful of them run
the real tools and are ignored by default, see `docs/development.md`), no frontend tests. Details per phase below,
this list is the index of what is still open.

**Release and CD**
- [x] First real `release.yml` run verified (see above); the release hook now only edits the version line of `tauri.conf.json` (it used to reformat the file with `jq`)
- [ ] Updater end to end: install v0.0.1 from its dmg (v0.1.0 is already published as the update), see the in-app update appear, install and relaunch. The manifest points at `api.github.com/.../releases/assets/<id>` URLs, which is what `tauri-action` writes: check that the updater downloads from them
- [ ] The changelog only lists `feat`, `fix`, `perf` and `revert` commits (angular preset): `chore` and `docs` stay out of it. It now shows the `## 0.0.1` heading twice (release-it writes the header again): find the cause or tidy by hand
- [ ] Code signing and notarization when the app goes to anyone else, including the nested JRE and Typst binaries

**Validation and safety**
- [ ] Bump Mustang when a release with veraPDF 1.30.3 exists (`scripts/sidecars.json`; 2.26.0 ships 1.30.2, advisory GHSA-5gvp-6g43-g47v)
- [ ] Test that fails if anything modifies the PDF after `combine` (no validator checks the attachment relationship)
- [ ] Check that Mustang makes no network calls (input is only our own files, so low risk)
- [ ] German texts for Mustang's report messages (the form's rule findings are German already)

**Invoice features not in the UI yet**
- [ ] Credit notes from the UI (the model and the template support them), several VAT rates, reverse charge and other VAT-free cases, discounts and surcharges
- [ ] Other payment means (direct debit, card), tax representative, payee, attachments (BG-24)
- [ ] Kleinunternehmer mode (§ 19 UStG), 0 % rate
- [ ] Customer-specific payment terms, mark as paid, dunning

**UI polish**
- [ ] Invoice list: paid status, more filters (year, customer) and sort options (search and a status filter exist)
- [ ] Position templates: use counts or favourites, templates with several lines
- [ ] Unsaved edits are saved as a draft when another invoice is opened, but not when the window closes: save on quit
- [ ] A readable maximum width for single-line inputs on very wide windows
- [ ] Frontend tests (flows were only checked by hand in the browser mock)
- [ ] Some rule texts may read oddly (e.g. `BR-CO-26` when only a tax number is set): collect and reword

**App icon**
- [x] The design lives in `src/assets/icon-default.icon` (Icon Composer; `icon-default.png` is its export, `icon-default.svg` the glyph layer, `icon-default.af` the Affinity source). `scripts/icons.ts` places the export on an 824 px body in a transparent 1024 px canvas (the margin macOS expects in an `.icns`) and runs `tauri icon`: that is the icon for macOS before 26 and for the window. Re-export the PNG from Icon Composer after changing the design
- [ ] The Liquid Glass icon itself: Tauri's bundler (2.10.3) compiles a `.icon` listed under `bundle.icon` with `actool` (Xcode 26+). With the installed Xcode 27 beta (27A266a) `actool` fails on every `.icon`, even a minimal one (`Failed to create a catalog from the Icon Studio document`, `Bad file descriptor`; reproduced outside Tauri and outside the sandbox), and the bundler treats that as a fatal error. Retry with a release Xcode 26.x or a later 27 build, then add `"../src/assets/icon-default.icon"` to `bundle.icon`. The CI runner's Xcode is untested: if it is tried there, a failed run is cleaned up by the `cleanup` job (draft and tag are removed)

**Repo cleanup**
- [ ] `src/assets/5632d75c-161f-49e1-83c7-c58c111ceaaf.png` is a stray personal photo that nothing references (it was committed with the docs): remove it
- [ ] `@enke.dev/lit-utils` is not used (the conventions below mention it as an option)

**Invoice template**
- [ ] Bundle a sans-serif font (Typst only embeds serif and math fonts; Inter is OFL)
- [ ] EPC / GiroCode QR code for EUR credit transfers (needs a vendored QR package, no network at compile time)
- [ ] Exact DIN 5008 window geometry and fold marks
- [ ] English invoices (labels live in `render/view.rs`)
- [ ] Seller register entry and managing directors as proper fields instead of free footer text

**Manual checks that only the real app can answer** (`bunx tauri dev`)
- [ ] The whole flow with the real backend: settings, customer, invoice, create and check, save as PDF, open the PDF and its embedded XML in a ZUGFeRD viewer
- [ ] The macOS title bar overlay and the header spacing next to the traffic lights
- [ ] Dark and light mode following the system, scrolling to the bottom of a long invoice
- [ ] An export with a logo, the updater button (hidden until a release exists)

## Architecture

Rust owns what is ours (model, rules, XML, rendering, storage, the export gate); the webview only edits forms. Everything standards-related that is not a rule on the model is delegated to **Mustang**, a battle-tested Java tool (its CLI bundles veraPDF and the official EN 16931 / XRechnung rule sets), shipped as a sidecar with a bundled JRE. No hand-written PDF/A or XMP logic.

```
form (webview) ─► InvoiceDraft ─► en16931::Invoice (draft.rs) ─► rule checks (German findings) ─► CII XML
                                                         └─► view (render/view.rs) ─► Typst ─► PDF/A-3b (plain, no attachment)
                                                                                              │
                        Mustang (sidecar, bundled JRE) ◄── combine: attach XML + write Factur-X XMP ─► ZUGFeRD PDF
                                                       └── validate: PDF/A-3b (veraPDF) + XML rules (EN 16931 / XRechnung)
```

Blocking gate, all in a per-invoice work directory (`pipeline.rs`, `session.rs`):

```
produce_invoice(id) ─► files + report in a temp dir ─► valid: UI offers "save as" ─► export_invoice copies, then finalizes (number, lock)
                                                    └─ invalid: report in UI, nothing leaves the directory
```

- The invoice model **and** its rules are the `en16931` crate's (`Invoice`, 334 rules, `Validated` proof before the CII writer runs). The rule check is the first gate, Mustang the final authority on the produced file
- The Mustang calls live in one module (`package.rs`: `combine`, `validate`), so a native implementation can replace them later without touching callers
- KoSIT validator: cross-check in Phase 1/2 and local tests only, not shipped
- The gate is enforced on the Rust side: `export_invoice` only copies files of a work directory whose report was valid and which belongs to the invoice
- Sidecar footprint (measured): Typst 43 MB + Mustang 56 MB + JRE 51 MB = 150 MB, dmg 116 MB; a validation takes about 1.5-2 s

## Verified facts (spike, 2026-10-16/17)

### Typst 0.15.1
- [x] PDF/A-3b/3u only via the `--pdf-standard a-3b` CLI flag (there is **no** `#set pdf(standard: ...)`)
- [x] `pdf.attach(...)` exists but writes no Factur-X XMP: not used, Mustang's `combine` does attachment and XMP
- [x] Typst cannot write custom XMP (`metadata.rs` writes title, authors, description, keywords, language, creator, ID, date; krilla has no hook), which is why the packaging step is Mustang's
- [x] Typst only embeds serif and math fonts (Libertinus Serif, New Computer Modern, DejaVu Sans Mono)

### lopdf (0.45.0): not used
The planned custom incremental XMP patch was never needed, Mustang takes the Typst output as is. Only relevant as a fallback if a future Mustang stops doing so.

## Phase 1 – Spike: end-to-end proof

Status: **done 2026-10-17, Mustang path confirmed.** Details, commands and numbers in [phase1-results.md](phase1-results.md). Fallback (custom lopdf patch, see Verified facts) is not needed.

- [x] Minimal Typst document, PDF/A-3b via `--pdf-standard a-3b`, **no** attachment
- [x] Sample XML (EN 16931) from the Mustang test resources
- [x] `mustang --action combine` on the Typst PDF → valid. Typst's own `pdf.attach` alone is invalid (no Factur-X XMP); combine on top of it works but embeds the XML twice, so do not use it
- [x] `mustang --action validate` → valid (PDF 3u via bundled veraPDF, XML, overall)
- [x] Standalone veraPDF 1.30.3 and KoSIT validator agree
- [x] `page` compared with veraPDF on the corpus and mutations: identical verdicts, 3 ms per file. Not a gate: it passes files Mustang rejects (missing Factur-X XMP). Pre-check candidate only
- [x] Corpus: 180 lines over 10 pages, long text, logo (SVG) + special glyphs, minimal → all valid
- [x] Sidecar measured: jar 59 MB, `jlink` JRE 51 MB (zip-9), about 1.4-2.1 s per validate, exit code 0 / 255
- [ ] Untrusted-input check of the sidecar (network, XXE) → move to Phase 4 hardening
- [ ] `page`'s own veraPDF corpus gate run locally (only if the pre-check is built)
- [x] Open questions:
  - [x] Combine keeps PDF/A conformance, but rewrites XMP B → U (A-3u); strict `3b` checkers complain
  - [x] Mustang covers EN 16931, XRechnung 3.0 (BR-DE-*) and Peppol rules; KoSIT stays a test cross-check. XRechnung sample with Leitweg-ID still to run in Phase 2
  - [x] Mustang 2.26.0 ships veraPDF 1.30.2 (GHSA-5gvp-6g43-g47v, fixed 1.30.3). Master already uses 1.30.3 → pin the next release, or build from master

Findings that shape later phases:
- No validator checks `/AFRelationship`; only `combine` sets it. Nothing may modify the PDF after combine
- Validate the **combined** file, never the Typst output
- Add negative tests (missing XMP, wrong relationship) to the golden set so a Mustang upgrade that changes behavior is noticed

## Phase 2 – Data model and XML

Status: **done 2026-10-17.** Built on the `en16931` and `en16931-formats` crates (v0.8.0, MIT OR Apache-2.0) instead of hand-written structs and XML: the crate's `Invoice` (serde-ready) is the single source of truth for UI, storage and XML, so there is no mirror model to keep in sync. Code in `src-tauri/src/invoice/`.

- [x] Model: `en16931::Invoice` with builder, parties, lines, allowances/charges, payment, delivery; `ProfileKind { En16931 (default), XRechnung }`, no MINIMUM/BASIC WL
- [x] Tax and rounding: crate's reconciler derives VAT breakdown and totals from the lines, 334 business rules (BR-*) in `validate()`
- [x] CII generation: `to_cii()` writes only after a proof of validity, BT-24 stamped from the profile
- [x] XRechnung variant: `BR-DE-*` rules (buyer reference / Leitweg-ID, seller contact, business process BT-23, spec id)
- [x] Golden tests (`cargo test`): standard, multiple rates 19/7, line + document allowances/charges, credit note, reverse charge, XRechnung incl. negative cases (missing buyer reference, wrong totals)
- [x] Cross-check against the authorities: all 6 generated files accepted by KoSIT (XRechnung config 2026-08-31) and valid in Mustang 2.26.0. Reproduce: `GOLDEN_OUT=<dir> cargo test`, then Mustang `--action validate` / KoSIT `-s scenarios.xml` on `<dir>`
- [x] Rule checks and CII writer wired up as `check_invoice` and the pipeline in Phase 4/5 (the first helper commands `validate_invoice_data` and `generate_cii` were replaced by those)

Findings that shape later phases:
- A line's net amount (BT-131) is taken as given: it must already include the line's allowances and charges, the crate does not recompute it. The form logic has to
- Always set a delivery date or period (also § 14 UStG): otherwise the mandatory CII delivery element is empty and Peppol rule R008 warns
- Mustang prints XRechnung/Peppol rules as notices for plain EN 16931 files (BR-DE-15, BR-DE-21, R001). Not errors: the gate blocks on errors only
- Severity matters in the UI: findings carry `fatal`, `warning`, `info`
- The crate asks derivative applications to carry the CEN attribution statement: `NOTICE.md`, show it in an About screen later
- The crate is young (3 months): the Mustang gate stays the authority, the golden set guards against crate upgrades

Form scope that is still open (also in the index at the top): further payment means (direct debit, card), tax representative, payee, attachments (BG-24), a full unit code list (UN/ECE rec. 20: the form offers five), the EAS scheme list for electronic addresses (the form always uses e-mail). Invoice numbering is done in Phase 5.

## Phase 3 – Typst template

Status: **done 2026-10-17** (first version, German only). Code in `src-tauri/src/render/`.

- [x] Data via JSON: the invoice is mapped to a presentation view (`render/view.rs`: strings only, German number/date formats, unit names, address lines, which totals to show), written as `data.json` next to the embedded `invoice.typ`; money is never computed in the template or the view
- [x] Layout: logo or seller name, DIN-5008-style sender line and recipient block, meta block (number, date, service date, due date, buyer reference, preceding invoice), line table with allowances/charges, totals per VAT rate, VAT-free statements (reverse charge etc. with a German default per category), payment block (terms, due date, IBAN/BIC, remittance info), footer with seller contact, VAT/tax id, free footer text and page numbers
- [x] § 14 UStG items covered when the data is there: seller and buyer name/address, VAT id or tax number, invoice number and date, service date or period, quantity/description/unit price, net per rate, rate and amount of VAT, VAT-free statements
- [x] Fonts embedded and PDF/A-compatible: Typst's own set, system fonts ignored for reproducible output (`--ignore-system-fonts`)
- [x] Integration: Typst CLI as sidecar (`--pdf-standard a-3b`), pinned and checksum-verified by `scripts/sidecars.ts`
- [x] Multi-page tested (70 lines, table header repeats, footer page numbers), umlauts and special glyphs fine

Evaluated and rejected: the Typst Universe package `classy-german-invoice` (MIT-0): one VAT rate computed inside the template, no quantities/units/allowances/credit notes/reverse charge/service date, two network packages fetched at compile time. Borrow the idea of an EPC (GiroCode) QR for the payment block.

Open:
- Bundle a sans-serif font (Typst only embeds serif and math fonts; Inter is OFL) via `--font-path`
- EPC/GiroCode QR code for EUR credit transfers (needs a vendored QR package, no network at compile time)
- Exact DIN 5008 window geometry and fold marks
- English (and other) invoice language: labels live in the view, a language switch would go there
- Seller registration extras (commercial register, managing directors) come only from the footer text today

## Phase 4 – Validation (in-app gate and tests)

Status: **done 2026-10-17**, UI for the report belongs to Phase 5. Code in `src-tauri/src/{sidecar,package,pipeline,session}.rs` and `commands/`.

Pipeline (`pipeline::produce`): rules check → CII XML → Typst PDF/A-3b → Mustang `combine` (Factur-X XMP + attachment, profile E or X) → Mustang `validate`, all in a per-invoice work directory.

- [x] Sidecars: `scripts/sidecars.ts` fetches Typst 0.15.1, Mustang 2.26.0 and a Temurin 21 JDK (sha256-pinned in `scripts/sidecars.json`), links a 51 MB JRE with `jlink` into `src-tauri/sidecars/`; `bundle.resources` ships the folder (verified: `tauri build --bundles app` contains it with exec bits, app 196 MB debug). `--stub` writes empty placeholders for lint/typecheck
- [x] Spawning from Rust (`sidecar::run`): no shell, no webview shell permission, timeout with kill, pipes drained on threads; Mustang with `-Xmx768m`, headless, UTF-8, no file logging
- [x] Typed report: Mustang's XML report (stdout; logs on stderr) parsed into issues (section pdf/xml/document, level error/warning/notice, rule id); `valid` only if the overall status is valid **and** the exit code is 0
- [x] Blocking gate enforced in Rust: `Sessions` registers each work directory with its verdict and invoice; `export_invoice` copies only files of a valid session of that invoice to the user-chosen targets, `discard_work_dir` and app exit delete the directories; the UI cannot bypass it
- [x] Commands: `produce_invoice(id)` (returns work dir, number and report, or input problems / invalid findings / failure), `export_invoice`, `discard_work_dir`
- [x] Golden set through the real tools: 7 cases (standard, 19/7 %, allowances + charges, 70 lines/multi-page, credit note, reverse charge, XRechnung incl. profile X), all valid; run with `bun run generate:sidecars && cargo test -- --include-ignored`
- [x] Negative tests: PDF/A without Factur-X XMP is rejected, tampered grand total is rejected (BR-CO-15); invalid invoices never reach a tool
- [x] CI: lint/typecheck/test use sidecar placeholders (ignored tests skipped), the release build restores and caches the real sidecars
- [ ] Hardening not done: the sidecar only ever reads files this app wrote itself (own XML, own PDF), so XXE and network paths are not reachable by outside input; still unverified that Mustang makes no network calls
- [ ] veraPDF inside Mustang 2.26.0 is 1.30.2 (advisory fixed in 1.30.3): bump Mustang when the release with 1.30.3 exists (`scripts/sidecars.json`)
- [ ] Codesigning/notarization of the nested JRE and Typst binaries when distributing (Tauri signs the main binary only)
- [ ] The attachment relationship is not checked by any validator: nothing may modify the PDF after `combine`; add a test that fails if the pipeline ever does

Findings:
- `InvoiceLine::new` takes the line's **net amount** and derives the unit price from it (3 × 12.50 shows up as `4,1666…`). Use `invoice::line_from_unit_price` (unit price kept, BT-131 = quantity × price rounded half away from zero to cents; line allowances/charges still to be added by the caller)
- A reverse-charge invoice needs its statement on the page: the model may carry only an exemption code, so the view falls back to a German text per VAT category
- Plain EN 16931 files get XRechnung notices from Mustang (notice level, not blocking)
- Sidecar footprint: Typst 43 MB + Mustang 56 MB + JRE 51 MB; jlink output is read-only and must be made writable or the second resource copy fails

## Phase 5 – Tauri app

Status: done 2026-10-17: storage, UI, export flow, CD and the first release (v0.0.1). What is left is in the index at the top.

Decisions (from the owner): one global tax rate (default 19 %, no 0 % / Kleinunternehmer mode yet); a position has a description, an optional quantity (empty = 1) and a **required** unit price; new and duplicated invoices start with today's date (customizable); exported invoices are locked, corrections go through "duplicate"; the UI is German, the invoice PDF too.

- [x] Scaffold per [App conventions](#app-conventions) (tooling, UI shell, Tauri layout): `check`, `lint`, `test`, `vite build`, `tauri dev` and `tauri build --bundles app` verified
- [x] CD: `.github/workflows/check.yml` + `release.yml`, `.release-it.json`, version sync hook: the first real run published v0.0.1 (all jobs green, about 14 minutes)
- [x] Storage: SQLite (`rusqlite`, bundled) in the app data directory, schema versioned by `user_version`, WAL; settings, customers and invoices (`src-tauri/src/store.rs`, 8 tests)
- [x] Draft → invoice mapping in one place (`draft.rs`): positions → lines via `line_from_unit_price`, seller/customer → parties, SEPA credit transfer, due date from the payment days, XRechnung business process; bad input is reported per field (`InputProblem`), rule violations as findings
- [x] Numbers: `<prefix><year>-<seq>`, e.g. `RE-2026-001`, assigned only when an invoice is exported (cancelled or invalid attempts never use one), unique in the database
- [x] Export flow: `produce_invoice(id)` (work dir + validated files) → UI shows the report → save dialogs → `export_invoice` copies the files, **then** finalizes the invoice (number, frozen settings, locked); a failed finalize deletes the copies; re-exporting a locked invoice reuses its number and frozen settings
- [x] Commands: `settings_get/save`, `customers_list`, `customer_save/delete`, `invoices_list`, `templates_list`, `template_save/delete`, `invoice_get/blank/save/duplicate/delete`, `check_invoice` (live totals, input problems and rule findings), `produce_invoice`, `export_invoice`, `discard_work_dir`
- [x] UI (German, Web Awesome): invoices as list and detail (see the next round), invoice editor (customer select + inline new customer, dates, XRechnung switch with Leitweg-ID, positions, note, live totals, problems/findings, export dialog with report and file choice), customers (cards + dialog), settings (company, bank, tax rate, payment days, number prefix, footer, logo)
- [x] Browser dev mode: `bun run vite dev` without Tauri installs a small mock backend (`common/utils/mock.utils.ts`, floating point totals, no rules) so the UI can be looked at; used to check list, editor, export dialog, lock, settings, customers
- [x] Auto-update: keypair generated (`~/.tauri/e-rng.key`, empty password, pubkey and `enke-dev/e-rng` endpoint in `tauri.conf.json`), updater component in the header (checks silently on start, a failed check only goes to the log), `publish-manifest` job created `gh-e-rng-latest`; secret `TAURI_SIGNING_PRIVATE_KEY` set in the repo (GitHub cannot store empty secrets, so no password variable is passed)
- [x] The repo is public, so the manifest needs no authentication
- [x] App icon from the Icon Composer export (see the index)
- [ ] Updater end to end and code signing / notarization: see the index at the top

List and detail round (2026-10-18):
- [x] Invoices are one view with a small list on the left and the selected invoice on the right (`/` and `/invoices/:id` render the same element, only its selection changes). The bare address redirects to the newest invoice (`redirect()` in `routing.utils.ts`), so a reload keeps the selection; `/invoices/new` shows a blank one; after a first save the new invoice is highlighted in the list; the list reloads on every `invoice-changed` event of the editor (save, export, duplicate, delete)
- [x] Shared `erng-filter-bar` (search field, slots for filters and actions): it lays itself out by the width of its container (container query, one column below 40 rem, one row above), so the same component is the sticky header of the narrow invoice list and the wide header of the customer and template cards
- [x] Position templates: a library of saved positions (description, quantity, unit, price) with cards and a dialog like the customers (page `Vorlagen`, SQLite table `templates`, schema version 2 with a migration test); the editor adds a position from a template ("Aus Vorlage …", searchable) and saves any position as one ("Als Vorlage")
- [x] Sortable positions: a drag handle with pointer events (`reorder.utils.ts`: the row follows the pointer, the other rows slide apart to open the gap where it would land with a dashed `.drop-slot` in it, the new index is applied on release; unequal row heights work; the target slot is the one nearest to the dragged row with a dead band of up to 12 px past the middle between two slots, so a swap needs a deliberate move back to be undone: measured 35 px to swap and 19 px back in both directions) and the arrow keys on the focused handle, focus follows the moved row. Pointer events instead of the HTML5 drag and drop API, which is unreliable inside webviews; checked with real mouse events in headless Chrome against the browser mock
- [x] Opening another invoice while a draft has unsaved edits saves the draft first
- [x] Positions: the header and the rows share the columns through `subgrid`, so the labels sit over the fields. When the minimum widths of the fields (declared in the grid) no longer fit, the fields stack, each with its own label; there is no breakpoint for this: a `ResizeObserver` notices that the grid overflows, remembers the width it needs (resolved columns plus gaps) and goes back as soon as that width is there (`measurePositions`). Checked by resizing from 1900 to 900 px and back: no overflow at any width, the switch happens at the same width in both directions

Second round (2026-10-18):
- [x] A new invoice is not in the list until it is saved as a draft (an earlier version showed a live "Entwurf" entry, removed again): while an unsaved `/invoices/new` is open the "Neue Rechnung" button is disabled, saving lists it, highlights it and enables the button; opening another invoice with unsaved edits saves them as a draft first. Messages of the editor ("Entwurf gespeichert", "gesperrt") are cleared when another invoice opens, so they cannot show up on a locked one
- [x] Buttons: never plain. Primary actions are `variant="brand"`, everything else `appearance="outlined"` (danger ones `variant="danger" appearance="outlined"`), related actions sit in a `wa-button-group` (position row actions, add position / from template, invoice header actions, save pair, dialog footers: cancel + save). The drag handle is an outlined `wa-button` now. `wa-button-group` only takes `wa-button`, no inputs
- [x] The shared search field has a lens button in the `end` slot of `wa-input` (docs: `start`/`end` slots take an element such as an icon or a button); the lens is an inline SVG, no icon library. Typing filters live, the button and Enter announce the query again. Fixed on the way: the attribute is `with-clear`, the earlier `clearable` did nothing
- [x] The list pane has a border on its right
- [x] Deleting an invoice removes it from the list at once and the address goes back to the bare `/`, which redirects to the newest remaining invoice (same logic as on entering); without any left a blank invoice shows
- [x] No spacing below scrollable areas: `main` has no bottom padding, the panes and views carry their own bottom padding inside the scroll area, so content scrolls to the window frame

Live preview (2026-10-18):
- [x] The invoice editor has a second column with the invoice as it will look, rendered by the real template: `preview_invoice(draft, id)` builds the invoice the same way as the export, renders it with the Typst sidecar to **SVG pages** (same `invoice.typ` and data as the PDF, no PDF/A) and returns them; the editor shows one page as an `<img>` with a pager for more. Requests are debounced (450 ms) and answered out of order only if newest, switching invoices drops the old preview; a real render takes about 0.1 s, a page is 200-450 KB of SVG
- [x] Half filled forms are previewed with stand-ins (`draft::for_preview`: price 0, description "–", today for a broken date), so the page is there from the first keystroke; the preview shows the number the invoice would get next (an exported invoice its own) and the frozen settings for locked invoices. Nothing is stored
- [x] The column has a readable width (20-36 rem) and is hidden when the pane is narrower than 62 rem (a fixed threshold, unlike the positions: revisit if it bothers); the positions adapt to whatever width the form is left with
- [x] Multi-page invoices: the renderer returns one SVG per page (the 70-line golden case gives 3), the editor shows one page with a pager (previous / "Seite x von y" / next, disabled at the ends); when an edit leaves fewer pages the shown page clamps instead of going blank. The page is sized by the height of the scrolling pane (the pane is a size container, `cqb`) minus the sticky footer (`--footer-height`, shared with the footer's own `min-height`), so page and pager never end up behind the footer, whatever the window height (checked at 700, 900 and 1300 px)
- [ ] The preview is the template, not the final file: the PDF/A conversion and the Factur-X attachment only happen on export, so the preview cannot show an export failure
- [ ] Zoom options for the page, keyboard paging (arrow keys), jumping to the page that was just edited; the browser mock only shows a rough placeholder, the real renderer is tested by `the_preview_renders_one_svg_per_page` (`RENDER_OUT=<dir>` keeps the SVGs)

Polish round (2026-10-17):
- [x] Rule findings in German: `invoice/german.rs` maps the 20 rules the form can trigger to German texts that say where to fix the problem; the English original stays as `hint`; a test breaks the form field by field and fails when a rule fires without a German text (also flags new rules after a crate upgrade). Rules the form cannot trigger, and Mustang's own report messages, stay English
- [x] Layout: the shell is a grid (header, scrolling main), so the bottom buttons are always reachable; views and cards use the full window width; the macOS header has extra top padding for the traffic lights
- [x] No scheme select, the app follows the system; the store plugin was removed with it

Not in the first UI version:
- Invoice list: no payment status, no year or customer filter
- Other payment means (direct debit, card), tax representative, payee, attachments (BG-24), discounts and surcharges, credit notes from the UI (the model supports them), several VAT rates, reverse charge
- A "mark as paid" / dunning workflow, a customer's own payment terms
- No unit tests for the frontend yet, UI flows were checked by hand in the browser mock

Manual checks in the real app (`bunx tauri dev`): the whole flow with the real backend (settings → customer → invoice → create and check → save as PDF → open the PDF and the embedded XML), the macOS title bar overlay, dark mode, the updater button (hidden until a release exists), an invoice export with a logo.

## App conventions

Adapted from an existing Tauri + Lit app (reference layout only, no code shared).

### Repo layout and tooling

```
.
├── .claude/plans/             this plan and the Phase 1 results
├── .configs/                  vite.config.ts, vite-watch-lit-css.plugin.ts
├── .github/workflows/         check.yml, release.yml
├── .husky/pre-commit          bunx --bun lint-staged
├── scripts/                   sidecars.ts + sidecars.json (pins), icons.ts
├── src/                       frontend (Lit)
├── src-tauri/                 Rust (Tauri 2)
├── README.md (short), docs/development.md, NOTICE.md
├── eslint.config.ts           re-exports @enke.dev/lint, ignores dist/ + src-tauri/target|gen
├── prettier.config.mjs        re-exports @enke.dev/lint prettier config
├── tsconfig.json              extends @tsconfig/strictest
├── lint-staged.config.ts      '*': () => 'bun run format'
├── .release-it.json
└── package.json
```

- **Package manager:** Bun (`.bun-version` pinned, `packageManager` field, `preinstall: bunx only-allow bun`); `bunfig.toml` with `exact = true` and `minimumReleaseAge = 86400`
- **TypeScript:** `@tsconfig/strictest`, `module: preserve`, `moduleResolution: bundler`, `experimentalDecorators` + `useDefineForClassFields: false` (Lit decorators), `exactOptionalPropertyTypes: false`, `noEmit`
- **Lint (TS):** `@enke.dev/lint` (eslint + prettier configs), `typescript-eslint`, `@awmottaz/prettier-plugin-void-html`; `eslint --config ./eslint.config.ts`
- **Lint (Rust):** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`, both with `--manifest-path src-tauri/Cargo.toml`
- **Scripts** (all run in parallel via `bun run --parallel`):

| Script | Does |
| --- | --- |
| `check` | `tsc --noEmit` |
| `lint` | `lint:rustfmt` + `lint:clippy` + `lint:eslint` |
| `format` | `format:rustfmt` + `format:eslint` (`eslint --fix`) |
| `test` | `test:vitest` + `test:cargo` in parallel: `vitest run --config .configs/vite.config.ts --passWithNoTests` and `cargo test --manifest-path src-tauri/Cargo.toml` |
| `generate` | `generate:icons` + `generate:sidecars` in parallel (both outputs are gitignored) |
| `generate:icons` | `scripts/icons.ts`: `tauri icon` on the Icon Composer export |
| `generate:sidecars` | `scripts/sidecars.ts`: downloads Typst, Mustang and a JDK (sha256-pinned), links the JRE; `:stub` writes empty placeholders (what CI uses) |
| `dev` / `build` | `tauri dev` / `tauri build` |

- **Pre-commit:** husky -> lint-staged runs `bun run format` once for the whole repo (not per file)
- **Tests:** vitest co-located (`*.test.ts`); Rust unit tests inline in `#[cfg(test)] mod tests`

### UI stack

- **Lit 3** + decorators (`@customElement`, `@property`, `@state`, `@eventOptions({ passive: true })`)
- **Web Awesome** (`@awesome.me/webawesome`, pin exact): buttons are brand or outlined, never plain, related ones in a `wa-button-group`; import each component individually (`.../components/button/button.js`), never the bundle; used: `wa-button`, `wa-input`, `wa-textarea`, `wa-select`/`wa-option`, `wa-checkbox`, `wa-switch`, `wa-button-group`, `wa-card`, `wa-dialog`, `wa-callout`, `wa-tag`, `wa-spinner` (no `wa-icon`: the default icon library would load from the network)
- **Theming:** global WA stylesheet imported in `src/styles/styles.css`; dark mode via `wa-dark` class on `<html>`, always following the system (no setting)
- **State:** `@lit-labs/signals` (`signal`, `computed`) in `src/state.ts`; base element = `SignalWatcher(LitElement)` so components re-render on signal reads; persistent data lives in SQLite on the Rust side, the webview keeps no storage
- **Routing:** `@lit-labs/router` (`RouteConfig[]`) in one `root.routes.ts` (`/`, `/invoices/:id`, `/customers`, `/templates`, `/settings`), views are imported lazily on first visit, `registerRouter(this, routes)` in the root view, `urlpattern-polyfill` for WebKit; `/invoices/:id` is `new` or an id
- **Base element:** one project base class (`ErngElement`) = `SignalWatcher(LitElement)` + base CSS (`:host { display: contents }`); every component extends it and sets `static styles: CSSResultGroup = [ErngElement.styles, styles]`
- **CSS:** one `.css` file per component, imported as a Lit `CSSResult` via `vite-plugin-lit-css` (`exclude: src/styles/**`); CSS nesting allowed; custom `litCssWatch` plugin for HMR of component CSS
- **HMR:** `vite-plugin-web-components-hmr` with `presets.lit`
- **Boot (`main.ts`):** polyfills (`element-internals-polyfill`, `urlpattern-polyfill`) -> `followSystemScheme()` -> in a plain browser the mock backend is installed -> `initLog()` -> on failure write to the `errors` signal -> then dynamic `import('./views/root.view.js')`; the root view removes the `#wait` boot spinner after its first update; the error state renders a `wa-dialog` with a reload button

### File layout (`src/`)

```
src/
├── main.ts
├── state.ts                              global signals (loading, errors, route) and followSystemScheme()
├── styles/                               styles.css, loading.css (boot spinner)
├── assets/                               icon-default.{icon,png,svg,af}
├── common/
│   ├── components/<name>/                address-fields, customer-form, filter-bar, updater (<name>.component.{ts,css})
│   └── utils/                            api (typed commands + shapes), base (ErngElement), format, form, log, routing, reorder, mock
└── views/
    ├── root.view.{ts,css}, root.routes.ts
    └── <view>/<view>.view.{ts,css}       invoices (list + detail), invoice-edit, customers, templates, settings
```

- Naming: kebab-case files with role suffix (`.component`, `.view`, `.routes`, `.utils`); custom element prefix `erng-`; `declare global { interface HTMLElementTagNameMap }` per element
- All commands go through the typed wrappers in `api.utils.ts`, the shapes there mirror `src-tauri/src/model.rs` by hand (snake_case JSON, money and quantities as strings)
- Logic that is worth testing without the DOM would go into a `*.data.ts` next to its view (none yet)

### Tauri layout (`src-tauri/`)

```
src-tauri/
├── Cargo.toml                lib `e_rng_lib` (staticlib, cdylib, rlib) + bin `e_rng`
├── build.rs
├── tauri.conf.json           resources: sidecars/, updater config, createUpdaterArtifacts
├── capabilities/
│   ├── default.json          windows ["*"]: core, dialog (open/save), fs temp, log, opener, os
│   └── desktop.json          platforms [macOS]: updater, process
├── sidecars/                 gitignored: typst, mustang.jar, jre/ (scripts/sidecars.ts)
└── src/
    ├── main.rs, lib.rs       plugins, managed state (Store, Sessions), invoke_handler![...]
    ├── model.rs              what is stored and edited (settings, customers, drafts, stored invoices)
    ├── store.rs              SQLite: settings, customers, invoices, numbers, finalize/lock
    ├── draft.rs              form draft → en16931 invoice, input problems, totals
    ├── invoice/              profile choice, rule checks (mod.rs), German finding texts (german.rs), golden test set (golden.rs)
    ├── render/               Typst template (invoice.typ) and the presentation view (view.rs)
    ├── package.rs            Mustang combine / validate, report parsing
    ├── pipeline.rs           rules → XML → PDF → combine → validate in a work directory
    ├── session.rs            work directories and their verdicts, the export gate
    ├── sidecar.rs            locating and running the bundled tools with a deadline
    ├── commands/             one tauri command (or a small family) per `<name>.command.rs`, registered via `#[path]` in mod.rs
    └── utils/                today(), message()
```

- Plugins registered in `lib.rs`: `fs`, `dialog`, `log`, `opener`, `os`, `process`, `updater` (desktop only, `cfg(not(any(android, ios)))`)
- Commands: `settings_get/save`, `customers_list`, `customer_save/delete`, `invoices_list`, `templates_list`, `template_save/delete`, `invoice_get/blank/save/duplicate/delete`, `check_invoice`, `preview_invoice`, `produce_invoice`, `export_invoice`, `discard_work_dir`
- Window: `titleBarStyle: "Overlay"`, `hiddenTitle`, `transparent`, `macOSPrivateApi`; drag region span rendered on macOS only
- Permissions: least privilege; no `http`, no `shell` plugin (the app is offline, tools are spawned from Rust)
- `tauri.conf.json`: `beforeDevCommand`/`beforeBuildCommand` run `generate` + vite; `devUrl` fixed port 1420 + `strictPort`; `frontendDist: ../dist`; `bundle.createUpdaterArtifacts: true`; `bundle.resources: {"sidecars/": "sidecars/"}`; `plugins.updater` with `active`, `pubkey`, `endpoints` (manifest on `gh-e-rng-latest`)
- Vite: `envPrefix: ['APP_', 'VITE_', 'TAURI_ENV_*']`, `build.target` safari13 (WebKit), `clearScreen: false`, ignore `**/src-tauri/**` in watch, `TAURI_DEV_HOST` for HMR
- Version: single source in `package.json`, synced to `Cargo.toml`, `Cargo.lock` and `tauri.conf.json` (version line only) by the release-it hook
- Gotcha: `jlink` output is read-only, `scripts/sidecars.ts` makes it writable, otherwise the second `tauri build` fails copying the resources

### CD

- **`check.yml`** (on PR + `workflow_call`): checkout -> setup Bun (`.bun-version`) + cache -> Rust stable (rustfmt, clippy) + `rust-cache` -> Tauri Linux deps (`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`) -> `bun install --frozen-lockfile` -> `generate:icons` + `generate:sidecars:stub` (placeholders, the tests that run the real tools are skipped) -> `check` -> `lint` -> `test` (vitest + cargo test).
- **`release.yml`** (push to `main`, `paths-ignore` for `.claude/**` and `**/*.md`), jobs in order:
  1. `check` – reuses `check.yml`
  2. `prepare` – skip if no commits since last tag; `release-it --ci --no-git` computes version + changelog (conventional commits, angular preset) + runs sync hook; output is a `bump.patch` artifact, nothing pushed yet
  3. `build` – single macOS aarch64 runner (`--target aarch64-apple-darwin`), no Intel, no Windows; apply patch, sccache + rust-cache, cache of `.cache/sidecars`, `tauri-apps/tauri-action` into a **draft** release (`beforeBuildCommand` runs `generate`, which fetches the sidecars)
  4. `release` – commit bump, push to `main`, move tag onto release commit, publish draft
  5. `cleanup` – on build failure delete draft + tag so the version is not consumed
  6. `publish-manifest` – take `latest.json` from the published release and push it to the `gh-e-rng-latest` branch (created on the first run) that the updater endpoint reads; no URL rewriting, the repo is public
- **Hardening:** actions pinned by commit SHA with version comment; least-privilege `permissions`; `concurrency` group so releases never overlap; `timeout-minutes` on every job
- **`.release-it.json`:** conventional-changelog plugin, `npm.publish: false`, `github.release: false` (tauri-action owns it), tag `v${version}`, `after:bump` hook syncs `Cargo.toml`, `Cargo.lock` and the version line of `tauri.conf.json`; the angular preset puts only `feat`/`fix`/`perf`/`revert` commits into the changelog
- **Sidecars:** `scripts/sidecars.ts` downloads Typst, Mustang and a Temurin JDK, verifies the pinned checksums and links the JRE, no tools need to be installed on the runner; the nested binaries must be signed with the app once notarization is set up
- **Secrets needed:** `TAURI_SIGNING_PRIVATE_KEY` (updater signing, public key goes into `tauri.conf.json`); Apple signing/notarization secrets only when distributing outside this machine

## Deadlines (Germany)

- Receiving e-invoices: mandatory since 2025
- Sending: paper/PDF still allowed until end of 2026
- From 2027: mandatory if prior-year revenue > €800,000
- From 2028: mandatory for everyone
- Small-amount invoices ≤ €250 exempt

## References

- Typst PDF docs: https://typst.app/docs/reference/pdf/
- Mustang: https://github.com/ZUGFeRD/mustangproject (CLI actions `combine`, `validate`; releases `core-x.y.z`)
- veraPDF: https://verapdf.org, advisory GHSA-5gvp-6g43-g47v (fixed in 1.30.3)
- KoSIT validator and the XRechnung configuration: https://github.com/itplr-kosit/validator, https://github.com/itplr-kosit/validator-configuration-xrechnung
- `en16931` crates: https://github.com/hupe1980/en16931 (model, rules, CII/UBL, MIT OR Apache-2.0)
- `page` (Rust PDF/A validator, evaluated as pre-check only): https://github.com/josephbarbierdarnal/page
- Web Awesome: https://webawesome.com/docs
- Tauri 2: https://v2.tauri.app/ (config, capabilities, `tauri-action`, Icon Composer `.icon` support in the bundler)
