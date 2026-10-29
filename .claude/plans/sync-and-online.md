# Sync, cloud and an online service – Plan

Goal: the data of e-rng is safe and reachable beyond one Mac: backups in the cloud first, then the same data on several devices, maybe one day e-rng as an online service.

Status: idea, not started. Builds on backup and restore (one `.erng-backup` file with the database snapshot, the originals and the layout files, see [docs/development.md](../../docs/development.md)). Each stage stands on its own; stop wherever it is enough.

## Stage 1: backups that leave the Mac (small)

- A backup folder in the settings, e.g. one in iCloud Drive, Dropbox, Nextcloud or a NAS: e-rng writes its backups there, the provider syncs the files. No account, no server, nothing of ours to run.
- Automatic backups: on quit, daily or weekly, keeping the last N plus one per month (GoBD wants years, the files are small).
- A reminder when the last backup is older than a set number of days.
- Optional encryption of the backup with a passphrase (age or AES-GCM with a key from Argon2), since invoices hold personal data (DSGVO) and the folder is someone else's cloud.
- Never put the live SQLite database into a synced folder: iCloud and the like copy it while it is written and break it (WAL files, partial uploads). Only finished backup files go there.

## Stage 2: the same data on several Macs (big)

- The hard part is not transport but the numbers: invoice and offer numbers must be unique and without gaps per sequence (GoBD), two devices must never issue the same number offline. Options:
  - one device is the issuer per context, others read only or write drafts,
  - numbers are only taken while online, from one place that counts (a lock or a server),
  - a device prefix in the pattern (`RE-A-2026-001`), allowed but odd for customers.
- Created documents are immutable, so they sync as append-only records with their originals; drafts, customers, saved positions, units, rates, layouts are editable and need conflict rules (last writer wins per record is enough for one person on two Macs).
- Transport candidates:
  - CloudKit (Apple only, free for the user, needs a native Tauri plugin in Swift and an Apple developer account),
  - a change log in a synced folder (works with any provider, conflicts by file, fragile),
  - our own sync server (see stage 3).
- iOS or iPadOS as a companion (look up invoices, mark paid) fits here, Tauri 2 builds for iOS.

## Stage 3: e-rng as an online service (product)

- The Lit frontend runs in a browser as it does in the webview; the Tauri commands become HTTP endpoints of a Rust server (axum), the store moves to Postgres with one schema or tenant id per account. Typst, Mustang and veraPDF run on the server (the sidecars are already separate processes).
- Accounts, login (passkeys), payment (Stripe or Paddle, invoices from e-rng itself), plans per number of contexts.
- Legal and trust:
  - hosting in Germany or the EU, DSGVO: a data processing agreement (AVV) with every customer, records of processing, deletion concepts,
  - GoBD: immutable storage with audit trail, a Verfahrensdokumentation, export for the tax office (Z3 data carrier access: the Steuerberater export plus originals), maybe an IDW PS 880 audit later,
  - the e-invoice duty for B2B in Germany (receiving since 2025, sending from 2027/2028) makes a receiving inbox (XRechnung/ZUGFeRD in, validated, archived) a strong feature of a service.
- Offline-first stays possible: the desktop app syncs with the service (stage 2 with our server as the counting place).
- Costs and duties of running it: operations, backups, security updates, support, liability. Worth a business plan before code.

## Order and first steps

1. Stage 1 in full (backup folder, automatic backups, reminder, encryption).
2. Decide whether more than one device is needed at all; if yes, the numbering rule first, then the transport.
3. Stage 3 only with a business case: who pays, for what, against which competitors (lexoffice, sevDesk, Debitoor/SumUp, FastBill).
