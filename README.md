# E-Rng

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

More: [`.claude/plans/init.md`](.claude/plans/init.md) (plan, decisions, open items) and
[`.claude/plans/phase1-results.md`](.claude/plans/phase1-results.md) (what the validators accept and reject).

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
  views/                   invoices, invoice-edit, customers, settings (+ root shell and routes)
  common/                  components, api.utils.ts (typed commands), format and form helpers, mock backend
src-tauri/src/
  invoice/                 profile choice, rule checks, German finding texts, CII writer, golden test set
  draft.rs                 form draft → invoice model
  store.rs, model.rs       SQLite and the stored shapes
  render/                  Typst template and the presentation view of an invoice
  package.rs               Mustang: combine and validate, report parsing
  pipeline.rs, session.rs  produce a validated file in a work directory, the export gate
  sidecar.rs               running the bundled tools with a deadline
  commands/                one tauri command per file
scripts/                   sidecars.ts and its pins (sidecars.json), icons.ts
```

## Release

Pushing to `main` runs `check`, then bumps the version (conventional commits, `release-it`), builds the app
on macOS into a draft release, commits the bump, publishes the release and writes `latest.json` for the
in-app updater to the `gh-e-rng-latest` branch. Needs the repo secret `TAURI_SIGNING_PRIVATE_KEY`.
Not signed or notarized yet.

## Licences

See [NOTICE.md](NOTICE.md): the e-invoice model comes from the `en16931` crates (CEN attribution),
Typst, Mustang (with veraPDF) and Temurin are shipped as separate programs.
