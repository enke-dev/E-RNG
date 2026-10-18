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
- Exported invoices are locked and keep the number, tax rate and seller details they were made with,
  corrections go through "duplicate". Numbers (`RE-2026-001`) are assigned on export only.

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

| Script | Does |
| --- | --- |
| `bun run check` | typecheck (`tsc`) |
| `bun run lint` | rustfmt, clippy (`-D warnings`), eslint |
| `bun run format` | rustfmt and eslint `--fix` (also runs on commit) |
| `bun run test` | vitest and `cargo test` |
| `bun run generate:sidecars:stub` | empty sidecar placeholders, enough for lint and typecheck (what CI does) |
| `bun run build` | `tauri build` |

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
  views/                   invoices (list + detail), invoice-edit, customers, templates, settings (+ root shell and routes)
  common/                  components (one folder each: `<name>/<name>.component.{ts,css}`, styles always in the css file, `ErngElement.withStyles(styles)`), layouts (split, cards, editor: slots only, views import no styles), api.utils.ts (typed commands), format and form helpers, mock backend
src-tauri/src/
  invoice/                 profile choice, rule checks, German finding texts, CII writer, golden test set
  draft.rs                 form draft → invoice model
  store.rs, model.rs       SQLite and the stored shapes
  render/                  invoice layouts (Typst) and the presentation view of an invoice
  layout_files.rs          own layouts: PDF stationery → SVG pages (hayro), font family from the file
  package.rs               Mustang: combine and validate, report parsing
  pipeline.rs, session.rs  produce a validated file in a work directory, the export gate
  sidecar.rs               running the bundled tools with a deadline
  commands/                one tauri command per file
scripts/                   sidecars.ts and its pins (sidecars.json), icons.ts
src-tauri/test-fixtures/   a font file for the layout tests (Open Sans, OFL)
```

## Data

The database and the layout files live in the app data directory (`~/Library/Application Support/dev.enke.e-rng`). A
debug build (`tauri dev`) uses `dev.enke.e-rng.dev` next to it, so a development run never migrates the data of an
installed app: an older version refuses a database a newer one has migrated ("written by a newer version").

## Icons

`<wa-icon name="save">` resolves against the library registered in `src/common/utils/icon.utils.ts`: a few Font Awesome
Free SVGs shipped with the app (the Web Awesome default library would load them from a CDN, which an offline app
cannot). Names say what an icon means (`save`, `trash`), add one by importing its svg there. Buttons with a label have no
icon; only the actions inside a list (remove a position, save it as a template) are icon only, with a `title` and an
`aria-label`.

## Own layouts

There is one built-in layout (`render/layouts/standard.typ`). A user layout is that layout plus stationery (PDF),
content margins and a font (table `layouts`, files in the app data directory). The default layout is a
setting; a user layout is deleted only while no invoice uses it and it is not the default, else it is archived
(out of the invoice's selection, kept for the invoices made with it). Typst refuses to embed a PDF in PDF/A output, so the PDF is converted to
SVG pages when picked (page 1 = first page, page 2 = all following, a single page = all). Margins are the
page margins; the first page may have its own on all four sides. Typst has one margin per page style, so
then the content is laid out in two runs (`two-runs` in `prelude.typ`): units (head, table rows, totals, …) are
measured and as many as fit go to the first page, the rest follows on pages with the other margins.
The settings show the same 30-position sample invoice in every layout, also the built-in one (`layout_preview`), with optional guides at the margins (red lines, `Branding.guides`). An exported invoice freezes the layout like the settings.
`cargo test -- --include-ignored` validates every golden case on stationery with an uploaded font.

## Release

Pushing to `main` runs `check`, then bumps the version (conventional commits, `release-it`), builds the app
on macOS into a draft release, commits the bump, publishes the release and writes `latest.json` for the
in-app updater to the `gh-e-rng-latest` branch. Needs the repo secret `TAURI_SIGNING_PRIVATE_KEY`.
Not signed or notarized yet.

## Licences

See [NOTICE.md](../NOTICE.md): the e-invoice model comes from the `en16931` crates (CEN attribution),
Typst, Mustang (with veraPDF) and Temurin are shipped as separate programs.
