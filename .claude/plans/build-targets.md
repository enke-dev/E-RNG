# Build targets – Plan

## Review decisions (2026-10-29) – they override the text below

- **Windows x86_64 only, no Intel Mac.** Everything about `x86_64-apple-darwin`, the Intel runner, Rosetta and the second DMG is dropped. The matrix is macOS aarch64 and Windows x86_64.
- **Unsigned on Windows for now**, the README explains SmartScreen ("Weitere Informationen" → "Trotzdem ausführen") and that Smart App Control blocks it.
- **Uninstalling never deletes the data**: the NSIS uninstaller's "delete app data" option is removed (installer hook / template), the database and the kept originals stay in `%APPDATA%\dev.enke.e-rng`.
- **Native title bar and Tauri's default menu** with the two app entries appended.
- The rest as proposed: NSIS `currentUser`, WebView2 `downloadBootstrapper`, `%APPDATA%` (Roaming), all-or-nothing releases. (Open questions 1–9 closed.)

## Review decisions (2026-10-30) – Windows is cross-built on Linux, overrides the text below

**Status (2026-10-30): implemented and tried.** Typst is linked into the app by now, the sidecars are only Mustang and the JRE (everything below about Typst binaries is obsolete). Done: per-target JDK pins and cross jlink (`sidecars.ts`, zips through `fflate`), `java.exe` and `CREATE_NO_WINDOW`, `dunce::simplified` on the resource dir (Java crashes on a `\\?\` jar path; the tests did not see it, they run from the repo), `/` in stored original paths, opened files from argv plus single instance, updater capability on Windows, `tauri.windows.conf.json`, the uninstall hook, `publisher`, `.gitattributes`, the release matrix with one draft created by POST (a list read right after does not have it yet) and `updater-manifest.ts`, the `release-try/**` dry run (keeps the bundles as workflow artifacts), README, docs, NOTICE. Tried on a physical Windows 11 (German): silent install, associations, Java, company, customer, invoice created with PDF and XML kept, second start and cold start with a layout file, no console window. Release time: the Windows leg (11.5 min) runs beside macOS (14 min); the Windows clippy and real-tool tests (15 min) are a job commented out in `check.yml`, off for now. Open: the uninstaller's checkbox (visible, overruled by the hook) tried interactively, SmartScreen on a downloaded copy, the updater between two releases.

- **The release builds Windows on `ubuntu-latest` with `cargo-xwin`**, as devkit does ([devkit release.yml](../../../devkit/.github/workflows/release.yml), the `windows (x64, cross)` leg): `tauri build --target x86_64-pc-windows-msvc --runner cargo-xwin`, apt `llvm clang lld nsis`, `makensis` linked as `makensis.exe` (Tauri looks for that name), `~/.cargo/bin/cargo-xwin` and `~/.cache/xwin` (the Windows SDK and CRT it downloads) cached. Linux runners are quicker than Windows ones; the repo is public, so the minutes are free either way. The only C code is SQLite (`rusqlite` `bundled`), which `cc` builds with clang-cl through xwin. The Rust cache is keyed per target.
- **jlink runs on the build host, against the target's jmods.** This replaces "jlink always runs from the target's own JDK, on the target OS": `sidecars.ts` downloads the host's JDK (for `jlink`) and the target's JDK (for its `jmods`) and runs `<host jdk>/bin/jlink --module-path <target jdk>/jmods`. Host and target are the same pinned version, which jlink requires (a mismatch fails the build, not the app). When host and target are equal it is one JDK, the code path is the same. The pins get a JDK for every host that builds: `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` (the check job below) and `x86_64-unknown-linux-gnu` (`OpenJDK21U-jdk_x64_linux_hotspot_21.0.12.1_1.tar.gz`, `sha256:ce79869e1307ed8ee1e2baa86a412b1eb5b75d10a01006d788a6f968bcfaee94`), which is only a build tool and never shipped. Typst needs no host build, only the target's binary is unpacked.
- **The target OS may differ from the host OS**: `sidecars.ts` refuses only a target it has no pins for. Unpacking the Windows zips on Linux and macOS uses `unzip` (on a Windows host the System32 `tar.exe`, as planned). Check that the jlinked `bin` of a cross-built image has the same files as one made on Windows (`java.exe`, `jli.dll`, the VC++ runtime DLLs) by building both once and comparing the file lists.
- **The real-tool smoke test is off for now, commented out at the end of `check.yml`** (at 15 minutes before every release too slow): a `windows-latest` job builds the sidecars natively and runs clippy for `--all-targets -D warnings` and `cargo test -- --include-ignored` (or the `pipeline::` and `creation::` subset, timed first). Linux cannot run `typst.exe` or `java.exe`, so the release legs only build. This job is also where the `cfg(windows)` code is first compiled, on every push, not in a release run.
- **Release matrix**: `macos-latest` / `aarch64-apple-darwin`, `ubuntu-latest` / `x86_64-pc-windows-msvc` (`--runner cargo-xwin`). The rest of the release flow as planned below (draft created once, `uploadUpdaterJson: false`, a manifest job, all or nothing).
- **The first try is a `workflow_dispatch` run** of the release workflow on a branch-free test tag or with publishing skipped (a `dry-run` input that stops before `release`), so the cross build and the manifest are proven before a real version is spent.
- **File types stay `.erng-layout` and `.erng-backup`.** `.json` and `.zip` were considered: the formats already are JSON and ZIP inside, but a generic extension cannot carry the file association (double-click import or restore) and would let any JSON or ZIP pass the open dialogs' filter. Both are associated (the backup since 2026-10-30); the Windows part of that (single instance, argv, NSIS registering both endings) stays in scope.

Goal: ship E-RNG for **macOS x86_64 (Intel)** and **Windows x86_64** next to macOS aarch64, from the same release, with the in-app updater on all three.

Status: planned 2026-10-28, nothing implemented. Facts marked *(verify)* were not confirmed against a run or the official docs.

## How it works today (for reference)

- `scripts/sidecars.ts` refuses anything but darwin/arm64. It downloads Typst (`tar.xz`), the Mustang jar and a Temurin JDK (`tar.gz`, checksum-pinned in `sidecars.json`). It runs `jlink` from that JDK into `src-tauri/sidecars/jre` and uses `chmod -R u+w`. A `.pins` stamp skips the work when nothing changed.
- The tools are a **resource folder** (`bundle.resources: { "sidecars/": "sidecars/" }`), not `externalBin`. `Sidecars::locate` looks in `resource_dir()/sidecars` and falls back to `CARGO_MANIFEST_DIR/sidecars` in dev. The paths are `typst`, `jre/bin/java` and `mustang.jar`, all without a file suffix.
- `sidecar::run` uses `std::process::Command` with no window flags.
- Typst only sees bare relative names (`invoice.typ`, `image-1.png`). Fonts come from `--font-path <dir>/fonts` with `--ignore-system-fonts`. Java gets an absolute `-jar` path and runs with the work folder as its current directory.
- `.erng-layout` files: `RunEvent::Opened` (macOS only) → `opened_files_command::offer(&[Url])`. The Windows and Linux paths are compiled out (`allow(dead_code)`).
- `capabilities/desktop.json`: `updater:default` and `process:default` for **`platforms: ["macOS"]` only**.
- `release.yml`: check → prepare (bump patch) → one macOS job with tauri-action (`releaseDraft`, `tagName`, updater JSON) → commit the bump, tag, publish → copy `latest.json` to `gh-e-rng-latest`. The current `latest.json` has `darwin-aarch64` and `darwin-aarch64-app`, with API asset URLs (`api.github.com/.../releases/assets/<id>`).
- The repo has no `.gitattributes`.

## Decisions

- **Two separate macOS builds, no universal app.** The JRE (`libjvm.dylib`, the launcher, `lib/modules`) is per arch. A universal app would mean running `lipo` on every Mach-O of two jlink outputs (not supported by Adoptium), or shipping both JREs and both Typst builds with a choice at runtime. That adds about 90 MB for everyone (today the JRE is 51 MB and Typst 43 MB). Separate builds keep the sidecar code identical per target. The cost: users have to pick the right DMG (`_aarch64` or `_x64`).
- **jlink always runs from the target's own JDK, on the target OS.** jlink and the jmods then always come from the same build, so their versions cannot mismatch, and the script has one code path. Cross-OS jlink is not needed, because a Windows bundle is built on a Windows runner anyway (`tauri build` cross-bundling for Windows is "last resort" in the Tauri docs). For macOS x86_64 on an Apple silicon host, the x64 JDK's `jlink` runs under Rosetta. That is the fallback once the Intel runner is gone. Plan B, should the runner not have Rosetta: host jlink with `--module-path <x64 jdk>/jmods`, which needs the exact same JDK version.
- **Resources stay a folder, no `externalBin`.** The JRE is a directory tree, which `externalBin` cannot express. `externalBin` would also need `typst-<triple>[.exe]` naming (and `typst-universal-apple-darwin` for universal builds), while Tauri strips the triple when it copies the binary next to the main executable. Instead, the target is chosen once, in `sidecars.ts`. `src-tauri/sidecars/` always holds the tools of the target being built.
- **Target selection in `sidecars.ts`:** `--target <triple>`, else `TAURI_ENV_TARGET_TRIPLE` (Tauri sets it for `beforeBuildCommand` and `beforeDevCommand`, confirmed in the CLI schema), else the host. The target OS must equal the host OS; a different arch is fine. The stamp covers the pins and the triple, so switching targets regenerates.
- **Windows installer: NSIS only, `installMode: both`.** The installer asks: the current user (no admin rights, `%LOCALAPPDATA%`) or all users (Program Files, UAC prompt, on updates too). Changed from `currentUser` after the v0.8.0 test: picking Program Files there failed with a bare "Fehler beim Überschreiben", the per-user installer cannot elevate. Updates remember the chosen mode; "only me" is preselected, and `windows/hooks.nsh` keeps v0.8.0 per-user installs (no mode marker yet) per user on update, where NSIS would move an admin's install to Program Files. Unlike MSI it does not need WiX, so it also builds on non-Windows hosts. MSI adds no value for a single-user app. German as the only installer language (`languages: ["German"]`, no language selector).
- **WebView2: keep the default `downloadBootstrapper`** (0 MB, needs internet only when WebView2 is missing). Windows 11 and current Windows 10 already have WebView2. `offlineInstaller` would add about 127 MB.
- **Windows code signing: none for now** (see Signing below). The updater's minisign signature does not depend on Authenticode and works unchanged.
- **Single instance on Windows and Linux** via `tauri-plugin-single-instance`, registered first. Not on macOS, where LaunchServices already delivers `RunEvent::Opened` to the running app, so macOS behaviour does not change.
- **One release, all or nothing.** If one target fails, the draft and tag are dropped as today (`fail-fast: true`). Every version is then available on every platform, and `latest.json` never names a version a platform lacks.
- **I build `latest.json` myself, not tauri-action.** tauri-action v1 (`upload-version-json.ts` at the pinned commit) reads the release's `latest.json`, merges its own platform, deletes the asset and uploads the new one, with no lock. With three parallel jobs a platform can be lost. The matrix runs with `uploadUpdaterJson: false`, and one job builds the manifest from the uploaded `.sig` assets after all builds. It writes the same keys tauri-action wrote (`{os}-{arch}` and `{os}-{arch}-{bundle}`; updater 2.14 looks up `{os}-{arch}-{installer}` first, then `{os}-{arch}`). Since v0.8.1 the URLs are the plain download links, not the API ones (`api.github.com/.../releases/assets/<id>`): those count against the 60 anonymous API requests an hour per address, and a Windows machine behind a shared one got 403.
- **The draft release is created once, before the matrix** (`gh release create --draft`). Its id goes to tauri-action as `releaseId`. Without that, three jobs looking up a draft by `tagName` at the same time could each create a draft *(verify; this avoids the question either way)*.
- **Stored original paths use `/`.** `originals.rs` stores `relative.to_string_lossy()`, which would be `invoices\1\RE-….pdf` on Windows. Writing with `/` now, before the first Windows release, means no migration. `Path::join` reads `/` on both OSes. Layout font and image paths are absolute and machine-bound already; that stays as it is.
- **Add `.gitattributes`** (`* text=auto eol=lf`, binary types marked). It makes Windows checkouts LF, so `bump.patch` from ubuntu applies and `include_str!` content is the same on every target.

## Pins (`scripts/sidecars.json`)

One version per tool, one URL and checksum per target, so the platforms cannot drift apart (same Typst means the same PDFs):

```json
{
  "typst": {
    "version": "0.15.1",
    "targets": {
      "aarch64-apple-darwin":   { "url": ".../v0.15.1/typst-aarch64-apple-darwin.tar.xz",   "sha256": "48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a" },
      "x86_64-apple-darwin":    { "url": ".../v0.15.1/typst-x86_64-apple-darwin.tar.xz",    "sha256": "7f9fdd9584866245de9a79e0add8f9236fae6f40a8a45e2c4771ccc14db4e0fa" },
      "x86_64-pc-windows-msvc": { "url": ".../v0.15.1/typst-x86_64-pc-windows-msvc.zip",    "sha256": "19ce3551153c2fe7ee9fa2f95208310c8f4d3209fedb699e0333faf8913f6736" }
    }
  },
  "mustang": { "version": "2.26.0", "url": "…", "sha256": "42d7868c…" },
  "jdk": {
    "version": "21.0.12.1+1",
    "modules": "java.base,…,jdk.xml.dom",
    "targets": {
      "aarch64-apple-darwin":   { "url": ".../OpenJDK21U-jdk_aarch64_mac_hotspot_21.0.12.1_1.tar.gz", "sha256": "3623232f33a9c3baadf304480b2535f9a3cba8a58d42ecbb438ba267315d9998" },
      "x86_64-apple-darwin":    { "url": ".../OpenJDK21U-jdk_x64_mac_hotspot_21.0.12.1_1.tar.gz",     "sha256": "44db0f08196daf19a47f90d13388b0c943b67663cb537f998fe29e836fa842ce" },
      "x86_64-pc-windows-msvc": { "url": ".../OpenJDK21U-jdk_x64_windows_hotspot_21.0.12.1_1.zip",    "sha256": "f9d6e191ab098c0d416e7d588a24420a8621cd2f4720dab2459b8b7b2d2d8b4e" }
    }
  }
}
```

The new values are the GitHub asset `digest` fields (the arm64 ones equal today's pins). The script checks them again on download anyway. Windows arm64 assets exist for both tools, so a fourth target later is only pins and one runner.

## Sidecar script (`scripts/sidecars.ts`)

- Resolve the target (see Decisions), and refuse a foreign OS with a clear message.
- Cache per target: `.cache/sidecars/<triple>/` (Mustang can stay shared).
- Extraction: `.tar.xz` and `.tar.gz` with `tar` as now. Windows `.zip` with `%SystemRoot%\System32\tar.exe` (bsdtar reads zip). On the GitHub Windows runner, a bare `tar` may resolve to Git's GNU tar, which cannot read zip *(verify which one comes first in `PATH`; calling the System32 one by its full path avoids the question)*. Check that the Typst zip has a top folder for `--strip-components=1` *(verify)*.
- File names: `typst.exe` and `jre/bin/java.exe` on Windows. The JDK home is `jdk-<v>/Contents/Home` on macOS and `jdk-<v>` on Windows.
- Replace `chmod -R u+w` with a walk using `fs.chmodSync`; on Windows that clears the read-only attribute.
- `--stub` writes the host's names (`typst.exe` on Windows).
- Keep the `jlink` flags as they are (`zip-9`, `--strip-debug` work on all three).

## Backend (src-tauri)

- `sidecar.rs`: `typst()` → `typst{EXE_SUFFIX}`, `java()` → `jre/bin/java{EXE_SUFFIX}` (`std::env::consts::EXE_SUFFIX`). In `run`, add `#[cfg(windows)] CommandExt::creation_flags(0x0800_0000)` (`CREATE_NO_WINDOW`). Without it, every Typst or Java call from the GUI app opens a console window. The current tests use `/bin/sh` and `/bin/sleep`: gate them with `#[cfg(unix)]` and add Windows versions using `cmd /C`.
- On Windows, `resource_dir()` is the folder of the exe (`tauri-utils` `resource_dir_from`), so `sidecars\` sits next to `E-RNG.exe`. No change is needed. Check that the `-jar` path has no `\\?\` prefix *(verify; `dunce::simplified` if it does)*.
- `opened-files.command.rs`: `offer(app, paths: impl IntoIterator<Item = PathBuf>)`. The filter takes paths, so macOS maps `Url::to_file_path` before calling it. The `cfg_attr(allow(dead_code))` attributes go.
- `lib.rs`:
  - `#[cfg(any(windows, target_os = "linux"))]` registers `tauri_plugin_single_instance::init(|app, argv, cwd| …)` as the **first** plugin. It skips `argv[0]`, makes relative paths absolute against `cwd`, calls `offer`, then unminimizes, shows and focuses the `main` window.
  - In `setup`, on the same platforms, offer `std::env::args_os().skip(1)` (cold start by double-click). Flags and non-layout paths are filtered out by the extension filter.
  - The Cargo dependency goes under `[target.'cfg(any(windows, target_os = "linux"))'.dependencies]`. The plugin needs Rust ≥ 1.90 per its docs.
- `originals.rs`: store paths joined with `/`.
- `capabilities/desktop.json`: `"platforms": ["macOS", "windows"]`.
- The frontend needs nothing for the file flow (`opened.utils.ts` already splits on `\` and `/`). Change the comments that say "Finder".

## Config

- `tauri.windows.conf.json` (merged over the main config; arrays are replaced as a whole):
  - `bundle.targets: ["nsis"]`
  - `bundle.windows.nsis: { installMode: "both", languages: ["German"], displayLanguageSelector: false }`
  - `bundle.windows.webviewInstallMode: { type: "downloadBootstrapper", silent: true }`
  - `app.windows`: the full window entry again, with `transparent: false` (`titleBarStyle` and `hiddenTitle` are macOS-only and ignored).
- Main config: `bundle.publisher: "David Enke"` (Apps & Features shows it), and the file association as it is (`role`, `rank` and `exportedType` are macOS-only; NSIS registers `.erng-layout` with `"%1"` as argument *(verify in the generated registry keys)*).
- `bundle.macOS.minimumSystemVersion`: set it to the strictest minimum of Tauri, Temurin 21 x64 and Typst x64 *(verify the Temurin 21 macOS minimum)*. macOS 26 is the last release for Intel Macs, so the Intel build never runs on anything newer.
- `bundleVCRuntime` stays `false` unless a clean VM shows `typst.exe` needing the VC++ runtime. Temurin ships its own runtime DLLs, and Tauri links the app statically by default.

## CI and release (`.github/workflows`)

- `release.yml` jobs:
  1. `check`, then `prepare` (unchanged).
  2. `create-release` (ubuntu) runs `gh release create v$V --draft --target main --title "E-RNG v$V" --notes <download hints>` and outputs the release id (looked up with `gh api …/releases`, filtered to the draft with that tag).
  3. `build`: a matrix with `fail-fast: true`, `defaults.run.shell: bash`:

     | name | runner | target |
     | --- | --- | --- |
     | macos aarch64 | `macos-latest` | `aarch64-apple-darwin` |
     | macos x86_64 | `macos-15-intel` | `x86_64-apple-darwin` |
     | windows x86_64 | `windows-latest` | `x86_64-pc-windows-msvc` |

     Steps:
     - Cache keys include the target: rust-cache `key: ${{ matrix.target }}`, sidecars `${{ matrix.target }}-sidecars-<hash>`, and bun `runner.os`-`runner.arch`.
     - `bun run generate:sidecars -- --target ${{ matrix.target }}`.
     - **Smoke test with the real tools**: `cargo test --target … -- --include-ignored` (or only `pipeline::` and `creation::` if the full set is too slow; time it first). This is the only place where Java and Typst are proven to run on Intel and Windows.
     - tauri-action with `releaseId`, `uploadUpdaterJson: false` and `args: --target ${{ matrix.target }}`.
  4. `manifest` (ubuntu): list the release assets and map them to platform keys. Fail if one of the three is missing.
     - `*_aarch64.app.tar.gz(.sig)` → `darwin-aarch64` and `darwin-aarch64-app`.
     - `*_x64.app.tar.gz(.sig)` → `darwin-x86_64` and `darwin-x86_64-app` *(verify the arch label tauri-action gives the x64 `.app.tar.gz`)*.
     - `*_x64-setup.exe(.sig)` → `windows-x86_64` and `windows-x86_64-nsis`.

     The signature is the content of the `.sig`, the URL the asset's API `url`. Upload `latest.json` as a release asset. This is a small `scripts/updater-manifest.ts` run by `bun`, consistent with the other scripts.
  5. `release` (needs `manifest`): commit the bump, tag, publish (unchanged).
  6. `publish-manifest` (unchanged).
  7. `cleanup`: `needs: [prepare, create-release, build, manifest]`.
- `check.yml`: add a `windows-latest` job running `cargo clippy --all-targets -D warnings` and `cargo test` against stub sidecars. Otherwise the `cfg(windows)` code (single instance, creation flags, `.exe` paths) is first compiled in a release run. The repo is public, so the minutes are free.
- Intel runner lifetime: `macos-15-intel` is described as the last x86_64 image, supported until about August or fall 2027 (secondary sources). After that, the x86_64 leg moves to `macos-latest` with the Rosetta jlink path. The smoke test then only runs if Rosetta is on the image.

## Signing and first start (unsigned)

- **macOS (both arches):** unchanged, signed ad hoc and not notarized. On macOS 15 and later, right-click → Open no longer gets past Gatekeeper. Fix the README: System Settings → Privacy & Security → "Open Anyway".
- **Windows SmartScreen:** the downloaded `-setup.exe` carries Mark-of-the-Web and gets "Windows protected your PC" → "More info" → "Run anyway". Every version is a new hash with no reputation, so expect the warning on every manual download. In-app updates are fetched by the app itself, so as far as I know they get no Mark-of-the-Web and no prompt *(verify)*.
- **Smart App Control** (Windows 11, on for some clean installs) blocks unsigned apps with no override. Those users cannot run E-RNG without turning it off.
- Defender occasionally flags unsigned NSIS installers (false positives).
- **Options for later:**
  - An OV certificate from a CA (about €150–300 a year, key on a hardware token or cloud HSM). EV certificates no longer skip SmartScreen reputation.
  - Azure Artifact Signing (formerly Trusted Signing). It accepts organizations in the EU, but individuals only from the US and Canada according to Microsoft's guidance *(verify current eligibility and price)*.
  - Either one plugs in via `bundle.windows.signCommand`.

## Docs

- README "Usage":
  - macOS Apple silicon: `E-RNG_<v>_aarch64.dmg`
  - macOS Intel: `E-RNG_<v>_x64.dmg`
  - Windows 10/11 64-bit: `E-RNG_<v>_x64-setup.exe`
  - The unsigned-app steps for both OSes, plus "macOS app" → "app for macOS and Windows".
- `docs/development.md`:
  - Requirements: build per target; Windows needs Bun, Rust (MSVC) and the VS Build Tools; same OS as the target.
  - Data: `%APPDATA%\dev.enke.e-rng` (Roaming), WebView2 data in `%LOCALAPPDATA%\dev.enke.e-rng`.
  - Layouts: the Windows file flow is wired.
  - Release: the matrix, the manifest job, the Intel runner end date.
  - Mention the NSIS uninstaller's "delete app data" checkbox (see Open questions).
- `NOTICE.md`: the Windows JRE contains Microsoft VC++ runtime DLLs from Temurin *(verify they are in the jlinked `bin`)*.

## Tests

- `sidecar.rs`: path names carry `.exe` on Windows. `run` captures output and exit code and kills on timeout on both OSes (`cfg(unix)` and `cfg(windows)` variants).
- Opened files:
  - The filter on paths: case-insensitive extension, other files dropped, flags (`--foo`) dropped.
  - Relative argv resolved against `cwd`.
  - Build the test paths from `std::env::temp_dir()`; today's `file:///tmp/…` fixture breaks on Windows.
- `originals.rs`: a stored path has `/` and reads back on the current OS.
- Release smoke test: the ignored real-tool tests on each target runner (see CI).
- `scripts/updater-manifest.ts`: refuses a release with a missing `.sig` or bundle, and maps the three asset names to the six keys. Run it against the 0.5.2 release as a dry run; it should reproduce today's two macOS keys.

## Manual checks

- **Each platform:**
  - Install from the release.
  - First-start warning and how to get past it.
  - Create an invoice (Typst and Mustang run with no console window flashing).
  - Preview, layout sample with an uploaded font, export PDF and XML, dark mode, window state.
- **Windows:**
  - Clean VM without the VC++ redistributable.
  - A user name with a space and an umlaut (Java converts arguments with the ANSI code page; the umlaut is fine in cp1252, other scripts are not *(verify)*).
  - Double-click `.erng-layout` with the app closed (cold start) and open (single instance, window comes to front).
  - The native menu bar (where "Nach Updates suchen" and "Einstellungen" end up, Ctrl+,).
  - Scrollbars and the header without the macOS padding.
  - Uninstall keeps `%APPDATA%\dev.enke.e-rng`.
  - On Windows 10, the WebView2 bootstrap when the runtime is missing.
- **Intel:** on a real Intel Mac. If none is at hand, at least the x64 build under Rosetta on Apple silicon, which does not prove Intel-only issues.
- **Updater, once the second multi-platform release is out:**
  - Each platform updates from N to N+1.
  - Windows: the app quits during install (the plugin calls `process::exit(0)`, so the frontend's `relaunch()` is not reached). NSIS gets `/UPDATE` plus restart args (`restart_after_install` defaults to true in updater 2.14), so the app should come back on its own.
  - Window state is probably not saved on that exit.
- **`codesign --verify --deep --strict`** on both DMGs' apps.

## Later

- Windows arm64 (`aarch64-pc-windows-msvc`; Typst and Temurin assets exist).
- Authenticode signing via `signCommand` once a certificate exists.
- macOS clippy leg in `check.yml` (the `RunEvent::Opened` code is only compiled in release builds today).
- "Im Explorer zeigen" next to the planned "Im Finder zeigen".

## Open questions

1. **Is the Intel build worth it?** macOS 26 is the last release for Intel Macs, and GitHub's Intel runner ends around 2027. Ship it, or only add Windows?
2. **Windows signing:** ship unsigned (SmartScreen on every download; Smart App Control users locked out), or buy an OV certificate? Would a business entity qualify for Azure Artifact Signing?
3. **NSIS install mode:** `currentUser` (proposed, no admin), or `both` so IT can install it for all users?
4. **WebView2:** `downloadBootstrapper` (proposed), `embedBootstrapper` (+1.8 MB), or `offlineInstaller` (+127 MB, works offline)?
5. **Windows menu bar:** keep Tauri's default window menu with the two entries appended, or no menu on Windows and both actions in the UI?
6. **Windows window:** native title bar (proposed), or a custom title bar to look like the macOS overlay?
7. **Data on uninstall:** the Tauri NSIS uninstaller offers to delete the app data (unchecked by default *(verify)*). That would remove the database and the kept invoice originals, which have to be kept for years. Accept it with a README warning, or override the uninstaller (`installerHooks` or a custom template)?
8. **Data folder:** `%APPDATA%` (Roaming, Tauri's `app_data_dir`, proposed), or `%LOCALAPPDATA%` so roaming profiles do not sync the database?
9. **All-or-nothing releases** (proposed), or publish what built and leave a platform behind?
