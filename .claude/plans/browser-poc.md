# e-rng in the browser – Plan

Goal: e-rng runs entirely in the browser, without a server and without Java: the logic in TypeScript instead of Rust (easier to maintain for us), Typst as wasm, the e-invoice rules from the official Schematron files. Desktop stays possible as a PWA or a thin Tauri shell without own logic.

Status: idea, a spike first. Starts after [vorgaenge.md](vorgaenge.md) is pushed. Related: [sync-and-online.md](sync-and-online.md) (stage 3 assumed a Rust server; this plan is the serverless alternative).

## Why not scriptc or Rust to wasm

- [scriptc](https://github.com/vercel-labs/scriptc) compiles TS to native executables or wasm, it does not turn Rust into TS; the browser runs TS as it is. Only useful later for a native CLI of the same code. Experimental, a subset of TS and Node.
- The Rust crates would compile to wasm (Typst, `rusqlite` on `sqlite-wasm-rs`, `en16931`), but the code stays Rust; we want TS.

## The parts and their browser counterpart

| Today | Browser |
|---|---|
| Typst crates (PDF/A-3b, SVG preview) | typst.ts (Typst as wasm, TS API), same Typst version, layouts unchanged |
| `en16931` model, CII via `en16931-formats` | own TS model and CII writer, ported from the current mapping; `@e-invoice-eu/core` as reference |
| `en16931::validate`, Mustang validate (Schematron) | SaxonJS running the official EN 16931 / XRechnung / ZUGFeRD Schematron (precompiled SEF) on our XML; one rules engine for checks while editing and at creation |
| Mustang combine (XML attachment, Factur-X XMP) | pdf-lib: attach `factur-x.xml` / `xrechnung.xml` as `text/xml`, AFRelationship `Alternative`, `/AF`, XMP with the Factur-X extension schema; watch PDF/A conformance |
| veraPDF in Mustang | none in the browser; Mustang and veraPDF stay in CI on golden invoices |
| `rust_decimal`, `phonenumber`, `zip`, `ttf-parser`, `imagesize` | decimal.js, libphonenumber-js, fflate, fontkit, `createImageBitmap` |
| SQLite via `rusqlite` | `@sqlite.org/sqlite-wasm` with the `opfs-sahpool` VFS in a dedicated worker; schema, migrations and JSON1 queries carry over |
| Tauri commands behind `invoke` | the same command names, handled by TS in the worker; `mock.utils.ts` becomes the real implementation |
| Tauri plugins (dialog, fs, opener, updater) | file inputs, downloads, File System Access API (Chromium), PWA updates |

## Storage

The engine is easy, durability is not: browser storage can be evicted, GoBD wants years.

- Working copy: SQLite wasm in OPFS. One tab holds the database (sahpool locks it), so tabs coordinate via Web Locks or BroadcastChannel.
- Durable copy: every created document's PDF and XML goes at once to a folder the user picks (File System Access API, Chromium; re-permission per session) or is downloaded (Safari, Firefox).
- Backups as today (`.erng-backup` zip), with reminders; stage 1 of [sync-and-online.md](sync-and-online.md) applies unchanged.
- `navigator.storage.persist()` and an installed PWA (Safari evicts script storage of sites not used for 7 days).
- Alternatives considered: IndexedDB/Dexie (no SQL, rewrite all queries), PGlite (heavier, no gain at our size), a sync server (no longer serverless, see sync plan stage 2/3).

## Spike (about a day, throwaway, own folder)

1. typst.ts renders one built-in layout with sample data to PDF/A-3b and SVG; compare against the Rust render.
2. sqlite-wasm in a worker opens the current schema (migrations up to 16) and runs one list query.
3. SaxonJS runs the EN 16931 CII and XRechnung Schematron on a golden XML; same findings as Mustang?
4. pdf-lib attaches the XML and XMP to the Typst PDF; Mustang validates the result (PDF/A included).
5. Measure: download size (wasm, fonts, SaxonJS, SEF files), cold start, time to create one invoice.

Blockers to look for: pdf-lib breaking PDF/A-3b, Schematron findings differing from Mustang, typst.ts lagging the Typst version we pin.

## Then (only if the spike holds)

1. Port the domain module by module with its tests (drafts, numbering, billing, on account, corrections, Vorgänge, history, backup); the Rust app stays the reference.
2. Port the creation pipeline; golden invoices give the same XML as Rust and pass Mustang.
3. Switch the frontend from Tauri `invoke` to the worker; Rust goes once both agree.

## Open

- Desktop: PWA only, or a thin Tauri shell (native file access, updater) without Rust logic?
- More than one device later? Then the numbering question of the sync plan first.
- Licences: SaxonJS is free but closed source; CheerpJ (whole Mustang in the browser) only as a fallback, paid beyond one-person companies.
