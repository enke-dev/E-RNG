# E-Invoice macOS App – Plan

Goal: Desktop app (Tauri) that generates a **ZUGFeRD/Factur-X PDF** (PDF/A-3 + embedded CII XML) and optionally an **XRechnung** (plain XML) from structured invoice data.

## Architecture

Rust owns what is ours (model, XML, rendering, UI, storage). Everything standards-related is delegated to **Mustang**, a battle-tested Java tool (its CLI bundles veraPDF and the official EN 16931 / XRechnung rule sets), shipped as a sidecar with a bundled JRE. No hand-written PDF/A or XMP logic, no re-implemented business rules.

```
Invoice data (Rust structs, single source of truth)
 ├─► CII XML (factur-x.xml)
 └─► JSON ─► Typst template ─► PDF/A-3b (plain, no attachment)
                                  │
   Mustang (sidecar, bundled JRE) ◄── combine: attach XML + write Factur-X XMP ─► ZUGFeRD PDF
                                  └── validate: PDF/A-3b (veraPDF) + XML rules (EN 16931 / XRechnung)
```

Blocking gate in the app, all in a per-invoice temp dir:

```
generate (temp) ─► Mustang validate ─► pass: save dialog copies temp ─► target
                                    └─ fail: report in UI, nothing leaves temp
```

- The Mustang calls live in one module (`package.rs`: `combine`, `validate`), so a native implementation can replace them later without touching callers
- `en16931` crate (Rust): optional, only for live field feedback in the invoice form, never a gate
- KoSIT validator: cross-check in Phase 1 and local tests only, not shipped
- Sidecar cost accepted: about 100 MB bundle (59 MB jar + about 40 MB `jlink` JRE), about 1-3 s per run

## Verified facts (as of 2026-10-16)

### Typst (`main`, 0.15.1)
- [x] PDF/A-3b/3u via the `--pdf-standard a-3b` CLI flag (**no** `#set pdf(standard: ...)` in 0.15.1, verified in the spike)
- [x] `pdf.attach(path, data, relationship: "alternative", mime-type:, description:)` – mapped to krilla `AssociationKind` in `crates/typst-pdf/src/attach.rs`
- [ ] **No** custom XMP: `metadata.rs` only writes title, authors, description, keywords, language, creator, ID, date
- [ ] krilla's `Metadata` API has no hook for custom XMP either
- ⚠️ For PDF/A-1..3, krilla already writes `<pdfaExtension:schemas><rdf:Bag>` → insert the Factur-X schema **into that Bag**, do not add a second `pdfaExtension:schemas`

### lopdf (`main`, 0.45.0, edition 2024, rust-version 1.88) – fallback only, if Mustang cannot take Typst output
- [x] `IncrementalDocument::load_from`, `get_prev_documents`, `opt_clone_object_to_new_document`, `new_document`, `save_to`
- [x] `Document::catalog`, `get_object`, `get_object_mut`
- [x] `Object::as_reference`, `as_stream`, `as_stream_mut` (macro-generated in `object.rs`)
- [x] `Stream::set_plain_content` removes `/Filter`, `/DecodeParms`, sets `/Length`

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
- [x] Tauri commands `validate_invoice_data` (live findings for the form) and `generate_cii`

Findings that shape later phases:
- A line's net amount (BT-131) is taken as given: it must already include the line's allowances and charges, the crate does not recompute it. The form logic has to
- Always set a delivery date or period (also § 14 UStG): otherwise the mandatory CII delivery element is empty and Peppol rule R008 warns
- Mustang prints XRechnung/Peppol rules as notices for plain EN 16931 files (BR-DE-15, BR-DE-21, R001). Not errors: the gate blocks on errors only
- Severity matters in the UI: findings carry `fatal`, `warning`, `info`
- The crate asks derivative applications to carry the CEN attribution statement: `NOTICE.md`, show it in an About screen later
- The crate is young (3 months): the Mustang gate stays the authority, the golden set guards against crate upgrades

Open for Phase 5 (form scope): further payment means (direct debit, card), tax representative, payee, attachments (BG-24), invoice numbering, unit code list (UN/ECE rec. 20), EAS scheme list for electronic addresses

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
- [x] Blocking gate enforced in Rust: `Sessions` registers each work directory with its verdict; `export_artifact` copies only files of valid sessions to a user-chosen target, `discard_work_dir` and app exit delete the directories; the UI cannot bypass it
- [x] Commands: `produce_invoice` (returns work dir + report, or invalid findings / failure), `export_artifact`, `discard_work_dir`
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

Status: UI and storage done 2026-10-17 (see below), CD/updater/first push still open.

Decisions (from the owner): one global tax rate (default 19 %, no 0 % / Kleinunternehmer mode yet); a position has a description, an optional quantity (empty = 1) and a **required** unit price; new and duplicated invoices start with today's date (customizable); exported invoices are locked, corrections go through "duplicate"; the UI is German, the invoice PDF too.

- [x] Scaffold per [App conventions](#app-conventions) (tooling, UI shell, Tauri layout): `check`, `lint`, `test`, `vite build`, `tauri dev` and `tauri build --bundles app` verified
- [x] CD: `.github/workflows/check.yml` + `release.yml`, `.release-it.json`, version sync hook (hook verified on a scratch copy; workflows only YAML-parsed, first real run happens after the first push)
- [x] Storage: SQLite (`rusqlite`, bundled) in the app data directory, schema versioned by `user_version`, WAL; settings, customers and invoices (`src-tauri/src/store.rs`, 8 tests)
- [x] Draft → invoice mapping in one place (`draft.rs`): positions → lines via `line_from_unit_price`, seller/customer → parties, SEPA credit transfer, due date from the payment days, XRechnung business process; bad input is reported per field (`InputProblem`), rule violations as findings
- [x] Numbers: `<prefix><year>-<seq>`, e.g. `RE-2026-001`, assigned only when an invoice is exported (cancelled or invalid attempts never use one), unique in the database
- [x] Export flow: `produce_invoice(id)` (work dir + validated files) → UI shows the report → save dialogs → `export_invoice` copies the files, **then** finalizes the invoice (number, frozen settings, locked); a failed finalize deletes the copies; re-exporting a locked invoice reuses its number and frozen settings
- [x] Commands: `settings_get/save`, `customers_list`, `customer_save/delete`, `invoices_list`, `invoice_get/blank/save/duplicate/delete`, `check_invoice` (live totals, input problems and rule findings), `produce_invoice`, `export_invoice`, `discard_work_dir`
- [x] UI (German, Web Awesome): invoice list with duplicate, invoice editor (customer select + inline new customer, dates, XRechnung switch with Leitweg-ID, positions, note, live totals, problems/findings, export dialog with report and file choice), customers (cards + dialog), settings (company, bank, tax rate, payment days, number prefix, footer, logo)
- [x] Browser dev mode: `bun run vite dev` without Tauri installs a small mock backend (`common/utils/mock.utils.ts`, floating point totals, no rules) so the UI can be looked at; used to check list, editor, export dialog, lock, settings, customers
- [x] Auto-update: keypair generated (`~/.tauri/e-rng.key`, empty password, pubkey and `enke-dev/e-rng` endpoint in `tauri.conf.json`), updater component in the header, `publish-manifest` job creates `gh-e-rng-latest` on first run; secret `TAURI_SIGNING_PRIVATE_KEY` set in the repo (GitHub cannot store empty secrets, so no password variable is passed)
- [ ] Auto-update, private repo only: manifest needs authenticated asset URLs and the updater a token
- [ ] Code signing + notarization only when distributing to others (see [CD](#cd), nested sidecar binaries need signing too)

Polish round (2026-10-17):
- [x] Rule findings in German: `invoice/german.rs` maps the 20 rules the form can trigger to German texts that say where to fix the problem; the English original stays as `hint`; a test breaks the form field by field and fails when a rule fires without a German text (also flags new rules after a crate upgrade). Rules the form cannot trigger, and Mustang's own report messages, stay English
- [x] Layout: the shell is a grid (header, scrolling main), so the bottom buttons are always reachable; views and cards use the full window width; the macOS header has extra top padding for the traffic lights
- [x] No scheme select, the app follows the system; the store plugin was removed with it

Not in the first UI version:
- Invoice list: no search, filter, sorting or payment status; editing positions has no reordering
- Other payment means (direct debit, card), tax representative, payee, attachments (BG-24), discounts and surcharges, credit notes from the UI (the model supports them), several VAT rates, reverse charge
- A "mark as paid" / dunning workflow, a customer's own payment terms
- No unit tests for the frontend yet, UI flows were checked by hand in the browser mock

Manual checks in the real app (`bunx tauri dev`): the whole flow with the real backend (settings → customer → invoice → create and check → save as PDF → open the PDF and the embedded XML), the macOS title bar overlay, dark mode, the updater button (hidden until a release exists), an invoice export with a logo.

## App conventions

Adapted from an existing Tauri + Lit app (reference layout only, no code shared).

### Repo layout and tooling

```
.
├── .configs/                  vite.config.ts, vite-watch-lit-css.plugin.ts
├── .github/workflows/         check.yml, release.yml
├── .husky/pre-commit          bunx --bun lint-staged
├── src/                       frontend (Lit)
├── src-tauri/                 Rust (Tauri 2)
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
| `generate:icons` | `tauri icon src/assets/icon-default.svg` (icons dir is gitignored) |
| `dev` / `build` | `tauri dev` / `tauri build` |

- **Pre-commit:** husky -> lint-staged runs `bun run format` once for the whole repo (not per file)
- **Tests:** vitest co-located (`*.test.ts`); Rust unit tests inline in `#[cfg(test)] mod tests`

### UI stack

- **Lit 3** + decorators (`@customElement`, `@property`, `@state`, `@eventOptions({ passive: true })`)
- **Web Awesome** (`@awesome.me/webawesome`, pin exact): import each component individually (`.../components/button/button.js`), never the bundle; used: `wa-button`, `wa-input`, `wa-select`/`wa-option`, `wa-combobox`, `wa-checkbox`, `wa-card`, `wa-dialog`, `wa-callout`, `wa-tab-group`, `wa-tooltip`, `wa-icon`, `wa-skeleton`, `wa-progress-bar`
- **Theming:** global WA stylesheet imported in `src/styles/styles.css`; dark mode via `wa-dark` class on `<html>`, always following the system (no setting)
- **State:** `@lit-labs/signals` (`signal`, `computed`) in `src/state.ts`; base element = `SignalWatcher(LitElement)` so components re-render on signal reads; persistent data lives in SQLite on the Rust side, the webview keeps no storage
- **Utilities:** `@enke.dev/lit-utils` for shared element helpers
- **Routing:** `@lit-labs/router` (`RouteConfig[]`), one `*.routes.ts` per view, `registerRouter(this, routes)` in the root view, `urlpattern-polyfill` for WebKit
- **Base element:** one project base class (`ErngElement`) = `SignalWatcher(LitElement)` + base CSS (`:host { display: contents }`); every component extends it and sets `static styles: CSSResultGroup = [ErngElement.styles, styles]`
- **CSS:** one `.css` file per component, imported as a Lit `CSSResult` via `vite-plugin-lit-css` (`exclude: src/styles/**`); CSS nesting allowed; custom `litCssWatch` plugin for HMR of component CSS
- **HMR:** `vite-plugin-web-components-hmr` with `presets.lit`
- **Boot (`main.ts`):** polyfills (`element-internals-polyfill`, `urlpattern-polyfill`) -> `Promise.all([initLog, initChecks, initRouting, initState, initTitle])` -> on failure write to `errors` signal -> then dynamic `import('./views/root.view.js')`; root view removes the `#wait` boot spinner after first update; error state renders a `wa-dialog` with a reload button

### File layout (`src/`)

```
src/
├── main.ts
├── state.ts                              global signals
├── styles/                               styles.css, loading.css (boot spinner)
├── assets/                               icon-default.svg
├── common/
│   ├── components/<name>/<name>.component.{ts,css}
│   └── utils/<topic>.utils.ts (+ .test.ts)
└── views/
    ├── root.view.{ts,css}, root.routes.ts
    └── <view>/<view>.{view,routes,data}.ts (+ .css, .data.test.ts)
```

- Naming: kebab-case files with role suffix (`.component`, `.view`, `.routes`, `.data`, `.utils`); custom element prefix = app short name (`<app>-root`, ...); `declare global { interface HTMLElementTagNameMap }` per element
- `*.data.ts` holds a view's pure logic (schemas, queries) so it is testable without the DOM

### Tauri layout (`src-tauri/`)

```
src-tauri/
├── Cargo.toml                lib `<app>_lib` (staticlib, cdylib, rlib) + bin
├── build.rs
├── tauri.conf.json
├── capabilities/
│   ├── default.json          windows ["*"], core + plugin permissions, scoped fs/http
│   └── desktop.json          platforms [macOS]: updater, process
└── src/
    ├── main.rs               calls `<app>_lib::run()`
    ├── lib.rs                plugins + invoke_handler![...]
    ├── commands/
    │   ├── mod.rs            `#[path = "x.command.rs"] pub mod x_command;` per file
    │   └── <name>.command.rs one `#[tauri::command]` each, logic in a private fn, tests inline
    └── utils/                shared helpers (+ test scratch dir helper)
```

- Plugins registered in `lib.rs`: `fs`, `dialog`, `log`, `opener`, `os`, `process`, `updater` (desktop only, `cfg(not(any(android, ios)))`)
- Window: `titleBarStyle: "Overlay"`, `hiddenTitle`, `transparent`, `macOSPrivateApi`; drag region span rendered on macOS only
- Permissions: least privilege, scope `fs` to specific dirs; only add `http` if needed (this app is offline -> skip)
- `tauri.conf.json`: `beforeDevCommand`/`beforeBuildCommand` run `generate` + vite; `devUrl` fixed port 1420 + `strictPort`; `frontendDist: ../dist`; `bundle.createUpdaterArtifacts: true`; `plugins.updater` with `active`, `pubkey`, `endpoints` (manifest on a dedicated branch)
- Vite: `envPrefix: ['APP_', 'VITE_', 'TAURI_ENV_*']`, `build.target` safari13 (WebKit), `clearScreen: false`, ignore `**/src-tauri/**` in watch, `TAURI_DEV_HOST` for HMR
- Version: single source in `package.json`, synced to `Cargo.toml`, `Cargo.lock`, `tauri.conf.json` by release-it hook

### CD

- **`check.yml`** (on PR + `workflow_call`): checkout -> setup Bun (`.bun-version`) + cache -> Rust stable (rustfmt, clippy) + `rust-cache` -> Tauri Linux deps (`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`) -> `bun install --frozen-lockfile` -> `check` -> `lint` -> `test` (vitest + cargo test).
- **`release.yml`** (push to `main`, paths filtered), jobs in order:
  1. `check` – reuses `check.yml`
  2. `prepare` – skip if no commits since last tag; `release-it --ci --no-git` computes version + changelog (conventional commits, angular preset) + runs sync hook; output is a `bump.patch` artifact, nothing pushed yet
  3. `build` – single macOS aarch64 runner (`--target aarch64-apple-darwin`), no Intel, no Windows; apply patch, sccache + rust-cache, `tauri-apps/tauri-action` into a **draft** release
  4. `release` – commit bump, push to `main`, move tag onto release commit, publish draft
  5. `cleanup` – on build failure delete draft + tag so the version is not consumed
  6. `publish-manifest` – take `latest.json` from the published release, rewrite asset URLs if the repo is private (authenticated API endpoints), push to a dedicated `gh-<app>-latest` branch that the updater endpoint reads
- **Hardening:** actions pinned by commit SHA with version comment; least-privilege `permissions`; `concurrency` group so releases never overlap; `timeout-minutes` on every job
- **`.release-it.json`:** conventional-changelog plugin, `npm.publish: false`, `github.release: false` (tauri-action owns it), tag `v${version}`, `after:bump` hook syncs Cargo/tauri.conf/lockfile
- **Sidecar:** `build` job also installs a JDK to `jlink` the JRE and fetches the pinned Mustang jar (checksum verified); sidecar binaries must be signed with the app on notarization
- **Secrets needed:** `TAURI_SIGNING_PRIVATE_KEY` (updater signing, public key goes into `tauri.conf.json`); Apple signing/notarization secrets only when distributing outside this machine

## Deadlines (Germany)

- Receiving e-invoices: mandatory since 2025
- Sending: paper/PDF still allowed until end of 2026
- From 2027: mandatory if prior-year revenue > €800,000
- From 2028: mandatory for everyone
- Small-amount invoices ≤ €250 exempt

## References

- Typst PDF docs: https://typst.app/docs/reference/pdf/
- Typst: `crates/typst-pdf/src/attach.rs`, `metadata.rs`
- krilla: `crates/krilla/src/configure/validate.rs` (extension schema), `chunk_container.rs` (XMP)
- lopdf: `src/incremental_document.rs`, `src/object.rs`
- Code sketch: `zugferd_xmp.rs`
- Web Awesome: https://webawesome.com/docs
- Tauri 2: https://v2.tauri.app/ (config, capabilities, `tauri-action`)
