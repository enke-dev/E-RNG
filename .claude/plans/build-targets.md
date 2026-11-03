# Build targets – Plan

**Status (2026-10-30): done and tried.** E-RNG ships for macOS aarch64 and Windows x86_64 from one
release, with the in-app updater on both. Details are in `docs/development.md` and the workflows;
this keeps the decisions and what is still open.

## Decisions

- **Targets**: macOS aarch64 and Windows x86_64. No Intel Mac (macOS 26 is the last for Intel, the
  runner ends around 2027), no universal app.
- **Windows is cross-built on `ubuntu-latest` with `cargo-xwin`**, as devkit does: apt `llvm clang lld
  nsis`, `makensis` linked as `makensis.exe`, xwin's SDK cache kept. The only C code is SQLite.
- **Sidecars are Mustang and a jlinked JRE** (Typst is linked into the app). jlink runs on the build
  host against the target's jmods, the same pinned JDK version for both (`scripts/sidecars.json`, a
  Linux JDK only as a build tool). Resources stay a folder, not `externalBin`.
- **Windows installer**: NSIS, `installMode: both` (only me preselected; `windows/hooks.nsh` keeps
  per-user installs per user on update), German only, WebView2 `downloadBootstrapper`.
- **Uninstalling never deletes the data**: the hook overrules the "delete app data" option, the
  database and the originals stay in `%APPDATA%\dev.enke.e-rng`.
- **Unsigned on Windows** for now; the README explains SmartScreen and that Smart App Control blocks
  it. The updater's minisign signature is independent of Authenticode.
- **Native title bar and Tauri's default menu** with the two app entries appended; single instance
  and opened files from argv on Windows.
- **One release, all or nothing**: the draft is created once by POST before the matrix,
  `uploadUpdaterJson: false`, and `scripts/updater-manifest.ts` builds `latest.json` from the uploaded
  `.sig` assets. URLs are plain download links built from the tag (the API ones hit the anonymous rate
  limit, the draft's ones are gone after publishing).
- **Stored original paths use `/`**, `.gitattributes` makes checkouts LF.
- **File types stay `.erng-layout` and `.erng-backup`**, both associated.
- **Release time**: the Windows leg (11.5 min) runs beside macOS (14 min). The Windows clippy and
  real-tool tests (15 min) are a job commented out in `check.yml`, too slow before every release.

## Open

- The uninstaller's checkbox (visible, overruled by the hook) tried interactively.
- SmartScreen on a downloaded copy.
- The updater from one multi-platform release to the next, on both platforms (Windows quits during
  install and should restart by itself).

## Later

- Windows arm64 (`aarch64-pc-windows-msvc`; the assets exist, it is pins and a runner).
- Authenticode signing via `bundle.windows.signCommand` once a certificate exists (an OV certificate;
  Azure Artifact Signing takes individuals only from the US and Canada).
- The Windows check job back on, if a cheaper subset of it is found.
- A macOS clippy leg in `check.yml` (the `RunEvent::Opened` code is only compiled in release builds).
- "Im Explorer zeigen" next to "Im Finder zeigen".
