//! Backup and restore of all data: the database, the kept originals of invoices and offers and the
//! files of own layouts, in one ZIP (`.erng-backup`) with a manifest that names every file with its
//! size and SHA-256. What comes back in is checked first, the data in use is replaced only when all of
//! it holds, and the data that was there is kept as a safety backup in `<data>/backups/`.

use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::originals;
use crate::store::{self, DataCounts, Store, SCHEMA_VERSION};

/// The database file in the data folder, and in a backup
pub const DB_FILE: &str = "e-rng.sqlite3";

/// What lives next to the database: the folders the originals and layout files are kept in
const FOLDERS: [&str; 3] = ["invoices", "offers", "layouts"];

/// Everything a restore replaces, as the names in the data folder (the write-ahead log and its
/// index only exist while the database is open, they belong to it)
const REPLACED: [&str; 6] = [
    DB_FILE,
    "e-rng.sqlite3-wal",
    "e-rng.sqlite3-shm",
    "invoices",
    "offers",
    "layouts",
];

const MANIFEST: &str = "manifest.json";
const FORMAT: &str = "e-rng-backup";
/// The layout of the file; a bigger number is a file of a newer app
const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "erng-backup";

/// Where the safety backups go, in the data folder, and how many of each kind stay
const SAFETY_DIR: &str = "backups";
const SAFETY_KEEP: usize = 3;

/// The manifest names no more than this many bytes, so a made-up file cannot fill the memory
const MANIFEST_LIMIT: u64 = 32 * 1024 * 1024;

const NEWER_VERSION: &str =
    "Diese Sicherung stammt aus einer neueren Version von e-rng. Bitte zuerst die App aktualisieren.";

/// One file of a backup
#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    /// Relative to the data folder, with `/`
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    app_version: String,
    schema_version: i32,
    /// UTC, like the times in the database
    created_at: String,
    files: Vec<Entry>,
}

/// Just what tells whether a file is a backup this app can read, before the rest is looked at
#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
}

/// A file to put into a backup, under its name there
struct Source {
    name: String,
    path: PathBuf,
}

/// Hashes and counts what passes through
struct Hashing<R> {
    inner: R,
    hasher: Sha256,
    len: u64,
}

impl<R: Read> Hashing<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            len: 0,
        }
    }

    fn finish(self) -> (u64, String) {
        (self.len, format!("{:x}", self.hasher.finalize()))
    }
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        self.len += read as u64;
        Ok(read)
    }
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn add_file<W: Write + Seek>(zip: &mut ZipWriter<W>, source: &Source) -> Result<Entry> {
    zip.start_file(&source.name, options())?;
    let file = File::open(&source.path)
        .with_context(|| format!("cannot read {}", source.path.display()))?;
    let mut reader = Hashing::new(file);
    io::copy(&mut reader, zip)?;
    let (size, sha256) = reader.finish();
    Ok(Entry {
        path: source.name.clone(),
        size,
        sha256,
    })
}

/// Writes the archive next to its target and moves it there when it is complete, so a failure
/// never leaves half a backup under the name of a good one. Returns the files and the size.
fn write_archive(
    target: &Path,
    sources: &[Source],
    schema_version: i32,
    app_version: &str,
    created_at: &str,
) -> Result<(usize, u64)> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("{} has no folder", target.display()))?;
    let mut partial = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("cannot write to {}", parent.display()))?;
    let mut zip = ZipWriter::new(partial.as_file_mut());
    let files = sources
        .iter()
        .map(|source| add_file(&mut zip, source))
        .collect::<Result<Vec<_>>>()?;
    let count = files.len();
    let manifest = Manifest {
        format: FORMAT.into(),
        version: FORMAT_VERSION,
        app_version: app_version.into(),
        schema_version,
        created_at: created_at.into(),
        files,
    };
    zip.start_file(MANIFEST, options())?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    zip.finish()?;
    partial.as_file().sync_all()?;
    let bytes = partial.as_file().metadata()?.len();
    partial
        .persist(target)
        .map_err(|error| error.error)
        .with_context(|| format!("cannot write {}", target.display()))?;
    Ok((count, bytes))
}

/// Every file below a folder of the data folder, in name order, by its name in a backup. Links
/// are not followed, the app makes none.
fn kept_files(base: &Path, relative: &str) -> Result<Vec<Source>> {
    let dir = base.join(relative);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = std::fs::read_dir(&dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let found = entries
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|name| anyhow!("the name {} is not valid text", name.to_string_lossy()))?;
            let name = format!("{relative}/{name}");
            let kind = entry.file_type()?;
            if kind.is_dir() {
                kept_files(base, &name)
            } else if kind.is_file() {
                Ok(vec![Source {
                    name,
                    path: entry.path(),
                }])
            } else {
                Ok(Vec::new())
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(found.into_iter().flatten().collect())
}

fn schema_of(database: &Path) -> Result<i32> {
    Ok(Connection::open(database)?.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// What a finished backup is
#[derive(Debug)]
pub struct Backup {
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub created_at: String,
}

/// Writes a backup of everything into `target`. The database is copied first, from one
/// transaction of the live connection; the files follow, so every file the copy knows is there
/// (a file that came in meanwhile is in the backup without its row, which does no harm).
/// `record` notes the time as the last backup, in the copy as well, so restored data knows when it
/// was backed up; the automatic safety copies do not count as one.
pub fn create(
    store: &Store,
    data: &Path,
    target: &Path,
    app_version: &str,
    record: bool,
) -> Result<Backup> {
    let work = tempfile::Builder::new()
        .prefix(".erng-backup-")
        .tempdir_in(data)?;
    let snapshot = work.path().join(DB_FILE);
    store.snapshot_to(&snapshot)?;
    let created_at = store::now();
    if record {
        store::stamp_snapshot(&snapshot, &created_at)?;
    }
    let schema_version = schema_of(&snapshot)?;
    let mut sources = vec![Source {
        name: DB_FILE.into(),
        path: snapshot,
    }];
    for folder in FOLDERS {
        sources.extend(kept_files(data, folder)?);
    }
    let (files, bytes) = write_archive(target, &sources, schema_version, app_version, &created_at)?;
    if record {
        store.set_last_backup_at(&created_at)?;
    }
    Ok(Backup {
        path: target.to_path_buf(),
        files,
        bytes,
        created_at,
    })
}

/// A name in a backup is one of the three folders and a file in it, or the database; never a way
/// out of the folder it is unpacked to
fn valid_name(name: &str) -> bool {
    if name == DB_FILE {
        return true;
    }
    let parts: Vec<&str> = name.split('/').collect();
    let in_folder = parts.first().is_some_and(|first| FOLDERS.contains(first));
    in_folder
        && parts.len() >= 2
        && !name.contains('\\')
        && parts
            .iter()
            .all(|part| !part.is_empty() && *part != "." && *part != "..")
        && Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn damaged(path: &str) -> anyhow::Error {
    anyhow!("Die Sicherung ist beschädigt: {path} stimmt nicht mit seiner Prüfsumme überein.")
}

fn read_manifest<R: Read + Seek>(zip: &mut ZipArchive<R>) -> Result<Manifest> {
    let mut entry = zip.by_name(MANIFEST).map_err(|_| {
        anyhow!("Das ist keine Datensicherung von e-rng: es fehlt die Datei {MANIFEST}.")
    })?;
    let mut text = Vec::new();
    // one byte more than the limit tells a file that is too big from one that just fits
    (&mut entry)
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut text)
        .map_err(|error| {
            anyhow!("Die Sicherung ist beschädigt: {MANIFEST} ist nicht lesbar ({error}).")
        })?;
    if text.len() as u64 > MANIFEST_LIMIT {
        bail!("Die Sicherung ist beschädigt: {MANIFEST} ist zu groß.");
    }
    let header: Header = serde_json::from_slice(&text)
        .map_err(|_| anyhow!("Das ist keine Datensicherung von e-rng: {MANIFEST} ist unlesbar."))?;
    if header.format != FORMAT {
        bail!("Das ist keine Datensicherung von e-rng.");
    }
    if header.version > FORMAT_VERSION {
        bail!(NEWER_VERSION);
    }
    serde_json::from_slice(&text).map_err(|error| {
        anyhow!("Die Sicherung ist beschädigt: {MANIFEST} ist unvollständig ({error}).")
    })
}

/// The files of the manifest are the files of the archive, no more and no fewer
fn check_listing<R: Read + Seek>(manifest: &Manifest, zip: &ZipArchive<R>) -> Result<()> {
    let mut names: Vec<&str> = manifest.files.iter().map(|e| e.path.as_str()).collect();
    if let Some(name) = names.iter().find(|name| !valid_name(name)) {
        bail!("Die Sicherung ist beschädigt: der Name {name} ist nicht erlaubt.");
    }
    names.sort_unstable();
    if let Some(name) = names.windows(2).find_map(|pair| match pair {
        [a, b] if a == b => Some(*a),
        _ => None,
    }) {
        bail!("Die Sicherung ist beschädigt: {name} steht doppelt darin.");
    }
    if !names.contains(&DB_FILE) {
        bail!("Die Sicherung ist unvollständig: die Datenbank fehlt.");
    }
    if let Some(extra) = zip
        .file_names()
        .filter(|name| !name.ends_with('/') && *name != MANIFEST)
        .find(|name| names.binary_search(name).is_err())
    {
        bail!("Die Sicherung ist beschädigt: {extra} steht nicht im Verzeichnis der Sicherung.");
    }
    Ok(())
}

/// Unpacks one file under `staging`, and checks size and hash on the way
fn extract<R: Read + Seek>(zip: &mut ZipArchive<R>, entry: &Entry, staging: &Path) -> Result<()> {
    let source = zip
        .by_name(&entry.path)
        .map_err(|_| anyhow!("Die Sicherung ist unvollständig: {} fehlt.", entry.path))?;
    let target = staging.join(&entry.path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // a file that claims less than it holds is cut one byte after its size, and fails the check
    let mut reader = Hashing::new(source.take(entry.size.saturating_add(1)));
    let mut out = File::create(&target)?;
    io::copy(&mut reader, &mut out).map_err(|error| {
        anyhow!(
            "Die Sicherung ist beschädigt: {} lässt sich nicht lesen ({error}).",
            entry.path
        )
    })?;
    let (size, sha256) = reader.finish();
    if size != entry.size || !sha256.eq_ignore_ascii_case(&entry.sha256) {
        return Err(damaged(&entry.path));
    }
    Ok(())
}

/// The unpacked database is one of this app: not newer, whole, and it migrates. It does so here,
/// in the staging folder, so a database that cannot be brought up to date is found before anything
/// in use is touched.
fn check_database(staging: &Path, manifest: &Manifest) -> Result<()> {
    let path = staging.join(DB_FILE);
    let broken = || anyhow!("Die Datenbank in der Sicherung ist beschädigt oder keine von e-rng.");
    let version: i32 = {
        let conn = Connection::open(&path).map_err(|_| broken())?;
        let version = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|_| broken())?;
        if version > SCHEMA_VERSION {
            bail!(NEWER_VERSION);
        }
        let report = conn
            .prepare("PRAGMA integrity_check")
            .and_then(|mut statement| {
                statement
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|_| broken())?;
        if report != ["ok"] {
            return Err(broken());
        }
        version
    };
    if version < 1 || version != manifest.schema_version {
        return Err(broken());
    }
    // opened like at the start of the app, and closed again
    Store::open(&path)
        .map(drop)
        .map_err(|error| anyhow!("Die Datenbank in der Sicherung lässt sich nicht auf den Stand dieser Version bringen: {error:#}"))
}

/// What a backup says of itself
#[derive(Debug)]
pub struct Prepared {
    pub created_at: String,
    pub app_version: String,
    /// The schema the backup was written with, before it was migrated
    pub schema_version: i32,
}

/// Unpacks a backup into `staging` and checks all of it: format, version, every hash, the database.
/// Nothing outside of `staging` is touched.
fn prepare(archive: &Path, staging: &Path) -> Result<Prepared> {
    let file = File::open(archive).with_context(|| format!("cannot open {}", archive.display()))?;
    let mut zip = ZipArchive::new(file).map_err(|_| {
        anyhow!("Das ist keine Datensicherung von e-rng: die Datei ist kein ZIP-Archiv.")
    })?;
    let manifest = read_manifest(&mut zip)?;
    check_listing(&manifest, &zip)?;
    manifest
        .files
        .iter()
        .try_for_each(|entry| extract(&mut zip, entry, staging))?;
    check_database(staging, &manifest)?;
    Ok(Prepared {
        created_at: manifest.created_at,
        app_version: manifest.app_version,
        schema_version: manifest.schema_version,
    })
}

/// What a swap did, to take back or to finish
struct Swapped {
    /// The renames, in order: from, to
    moved: Vec<(PathBuf, PathBuf)>,
    /// Where the replaced data went
    aside: PathBuf,
}

impl Swapped {
    fn undo(self) {
        undo(&self.moved);
        let _ = std::fs::remove_dir(&self.aside);
    }

    /// The replaced data goes. A safety backup of it was made, which is the way back now.
    fn finish(self) {
        let _ = std::fs::remove_dir_all(&self.aside);
    }
}

fn undo(moved: &[(PathBuf, PathBuf)]) {
    moved.iter().rev().for_each(|(from, to)| {
        let _ = std::fs::rename(to, from);
    });
}

fn move_all(from: &Path, to: &Path, moved: &mut Vec<(PathBuf, PathBuf)>) -> Result<()> {
    REPLACED.iter().try_for_each(|name| {
        let (source, target) = (from.join(name), to.join(name));
        if source.symlink_metadata().is_err() {
            return Ok(());
        }
        std::fs::rename(&source, &target)
            .with_context(|| format!("cannot move {} to {}", source.display(), target.display()))?;
        moved.push((source, target));
        Ok(())
    })
}

/// Moves the data in use aside and the staged data in its place, by renames within one folder.
/// When one of them fails the ones before are taken back and the data stays as it was.
fn swap(data: &Path, staging: &Path, aside: &Path) -> Result<Swapped> {
    std::fs::create_dir_all(aside)?;
    let mut moved = Vec::new();
    let result =
        move_all(data, aside, &mut moved).and_then(|()| move_all(staging, data, &mut moved));
    match result {
        Ok(()) => Ok(Swapped {
            moved,
            aside: aside.to_path_buf(),
        }),
        Err(error) => {
            undo(&moved);
            let _ = std::fs::remove_dir(aside);
            Err(error.context(
                "Die Daten konnten nicht ausgetauscht werden, die bisherigen sind unverändert.",
            ))
        }
    }
}

/// Puts what is in `staging` in place of the data in use, under the lock of the store
fn replace_data(store: &Store, data: &Path, staging: &Path, aside: &Path) -> Result<()> {
    store
        .replace_files(
            &data.join(DB_FILE),
            || swap(data, staging, aside),
            Swapped::undo,
        )?
        .finish();
    Ok(())
}

/// The kinds of safety backup, each with a name of its own and its own three
#[derive(Debug, Clone, Copy)]
enum Reason {
    Restore,
    Remove,
}

impl Reason {
    /// The end of the name, after the time it starts with so the files sort by time
    fn suffix(self) -> &'static str {
        match self {
            Self::Restore => "-vor-wiederherstellung",
            Self::Remove => "-vor-entfernen",
        }
    }
}

/// Where the safety backups are kept
pub fn safety_dir(data: &Path) -> PathBuf {
    data.join(SAFETY_DIR)
}

/// Deletes all but the newest safety backups of a kind. The names sort like time does.
fn rotate(dir: &Path, reason: Reason) {
    let suffix = format!("{}.{EXTENSION}", reason.suffix());
    let Ok(listing) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = listing
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(&suffix))
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a));
    names.iter().skip(SAFETY_KEEP).for_each(|name| {
        let _ = std::fs::remove_file(dir.join(name));
    });
}

/// A backup of the data in use, made before it is replaced. When it fails nothing is replaced.
fn safety_backup(store: &Store, data: &Path, reason: Reason, app_version: &str) -> Result<PathBuf> {
    let dir = safety_dir(data);
    std::fs::create_dir_all(&dir)?;
    let name = format!(
        "{}{}.{EXTENSION}",
        chrono::Local::now().format("%Y-%m-%d-%H%M%S"),
        reason.suffix()
    );
    let created = create(store, data, &dir.join(name), app_version, false).context(
        "Die automatische Sicherung der bisherigen Daten ist fehlgeschlagen, es wurde nichts verändert.",
    )?;
    rotate(&dir, reason);
    Ok(created.path)
}

/// A name for the folder the replaced data waits in
fn aside_dir(data: &Path) -> PathBuf {
    data.join(format!(
        ".erng-replaced-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ))
}

fn staging_dir(data: &Path) -> Result<tempfile::TempDir> {
    Ok(tempfile::Builder::new()
        .prefix(".erng-restore-")
        .tempdir_in(data)?)
}

/// What a restore did
#[derive(Debug)]
pub struct Restored {
    pub counts: DataCounts,
    pub safety_copy: PathBuf,
    /// What is wrong with the kept originals now: files that are missing or changed
    pub problems: Vec<String>,
    pub created_at: String,
    pub app_version: String,
    /// The schema the backup had, the data is at the current one now
    pub schema_version: i32,
}

/// Replaces all data by the contents of a backup. The file is checked completely first; then the
/// data in use is backed up (`…-vor-wiederherstellung`), then swapped; the kept originals are
/// checked against their hashes at last.
pub fn restore(store: &Store, data: &Path, archive: &Path, app_version: &str) -> Result<Restored> {
    let staging = staging_dir(data)?;
    let prepared = prepare(archive, staging.path())?;
    let safety_copy = safety_backup(store, data, Reason::Restore, app_version)?;
    replace_data(store, data, staging.path(), &aside_dir(data))?;
    Ok(Restored {
        counts: store.counts()?,
        safety_copy,
        problems: originals::problems(data, &store.original_files()?),
        created_at: prepared.created_at,
        app_version: prepared.app_version,
        schema_version: prepared.schema_version,
    })
}

/// What removing everything did
#[derive(Debug)]
pub struct Removed {
    pub safety_copy: PathBuf,
}

/// Resets the app to a first start: an empty database and no files. A safety backup
/// (`…-vor-entfernen`) is made first, and the data is not touched when that fails.
pub fn remove_all(store: &Store, data: &Path, app_version: &str) -> Result<Removed> {
    let safety_copy = safety_backup(store, data, Reason::Remove, app_version)?;
    let staging = staging_dir(data)?;
    // opened to be made, and closed again
    Store::open(&staging.path().join(DB_FILE)).map(drop)?;
    replace_data(store, data, staging.path(), &aside_dir(data))?;
    Ok(Removed { safety_copy })
}

/// Clears what a run that was cut off left in the data folder: unpacked backups and database
/// copies. The folder of replaced data stays, it may be the only copy of data that was lost.
pub fn clean_leftovers(data: &Path) {
    let Ok(listing) = std::fs::read_dir(data) else {
        return;
    };
    listing
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(".erng-restore-") || name.starts_with(".erng-backup-")
        })
        .for_each(|entry| {
            let _ = std::fs::remove_dir_all(entry.path());
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::draft::fixtures::{draft, settings};
    use crate::model::{CustomLayout, Kind};
    use crate::store::Sequence;

    const APP: &str = "0.0.0-test";

    fn bytes(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    fn write(data: &Path, name: &str, content: &[u8]) {
        let path = data.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// A data folder with a context, an exported invoice with its originals, an offer, an own
    /// layout with a picture and a font
    fn filled(data: &Path) -> Store {
        let store = Store::open(&data.join(DB_FILE)).unwrap();
        let context = store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap()
            .id
            .unwrap();
        let work = tempfile::tempdir().unwrap();
        let pdf = work.path().join("a.pdf");
        std::fs::write(&pdf, b"%PDF invoice").unwrap();
        let xml = work.path().join("a.xml");
        std::fs::write(&xml, b"<xml/>").unwrap();
        let produced = crate::pipeline::Produced {
            pdf,
            xml,
            report: crate::package::ValidationReport {
                valid: true,
                issues: Vec::new(),
            },
        };
        let files = originals::store(data, context, "RE-2026-001", &produced, false).unwrap();
        let id = store
            .save_draft(None, context, Kind::Invoice, &draft(), None)
            .unwrap()
            .id;
        let sequence = Sequence {
            next_number: "RE-2026-001".into(),
            counter: 1,
            highest: None,
            floor: None,
        };
        store
            .finalize(id, &sequence, &settings(), None, "10.00", &files)
            .unwrap();
        let offer =
            originals::store_pdf(data, Kind::Offer, context, "AN-2026-001", &produced.pdf).unwrap();
        let offer_id = store
            .save_draft(None, context, Kind::Offer, &draft(), None)
            .unwrap()
            .id;
        let sequence = Sequence {
            next_number: "AN-2026-001".into(),
            counter: 1,
            ..sequence
        };
        store
            .finalize(offer_id, &sequence, &settings(), None, "10.00", &offer)
            .unwrap();
        store
            .save_custom_layout(&CustomLayout {
                id: None,
                name: "Briefkopf".into(),
                fonts: Vec::new(),
                ..crate::layout::builtin()
            })
            .unwrap();
        write(data, "layouts/images/logo.png", b"not really a picture");
        write(data, "layouts/fonts/Sans-Regular.ttf", &[0, 1, 2, 3, 255]);
        store
    }

    /// Every file under the data folder that a backup holds, with its bytes
    fn kept(data: &Path) -> Vec<(String, Vec<u8>)> {
        FOLDERS
            .iter()
            .flat_map(|folder| kept_files(data, folder).unwrap())
            .map(|source| (source.name, bytes(&source.path)))
            .collect()
    }

    fn counts(contexts: usize, invoices: usize, offers: usize, layouts: usize) -> DataCounts {
        DataCounts {
            contexts,
            invoices,
            offers,
            layouts,
        }
    }

    fn backup_of(store: &Store, data: &Path, to: &Path) -> PathBuf {
        let target = to.join("test.erng-backup");
        create(store, data, &target, APP, true).unwrap();
        target
    }

    /// The archive again, every file passed through `change` (none: left out)
    fn rewritten(from: &Path, to: &Path, mut change: impl FnMut(&str, Vec<u8>) -> Option<Vec<u8>>) {
        let mut source = ZipArchive::new(File::open(from).unwrap()).unwrap();
        let mut zip = ZipWriter::new(File::create(to).unwrap());
        (0..source.len()).for_each(|index| {
            let mut entry = source.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut content = Vec::new();
            entry.read_to_end(&mut content).unwrap();
            if let Some(content) = change(&name, content) {
                zip.start_file(&name, options()).unwrap();
                zip.write_all(&content).unwrap();
            }
        });
        zip.finish().unwrap();
    }

    fn manifest_of(archive: &Path) -> Manifest {
        let mut zip = ZipArchive::new(File::open(archive).unwrap()).unwrap();
        read_manifest(&mut zip).unwrap()
    }

    fn refusal(store: &Store, data: &Path, archive: &Path) -> String {
        let before = kept(data);
        let error = restore(store, data, archive, APP).unwrap_err();
        // refused before anything was touched: no safety copy, the data is as it was
        assert!(!safety_dir(data).exists(), "{error:#}");
        assert_eq!(kept(data), before);
        format!("{error:#}")
    }

    #[test]
    fn a_backup_restores_everything_byte_for_byte() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let before = kept(data.path());
        assert_eq!(before.len(), 5);
        assert_eq!(store.counts().unwrap(), counts(1, 1, 1, 1));
        let archive = backup_of(&store, data.path(), out.path());
        assert!(store.last_backup_at().unwrap().is_some());

        // everything gone
        let removed = remove_all(&store, data.path(), APP).unwrap();
        assert!(removed.safety_copy.is_file());
        assert_eq!(store.counts().unwrap(), counts(0, 0, 0, 0));
        assert!(kept(data.path()).is_empty());
        assert_eq!(store.last_backup_at().unwrap(), None);

        let restored = restore(&store, data.path(), &archive, APP).unwrap();

        assert_eq!(kept(data.path()), before);
        assert_eq!(restored.counts, counts(1, 1, 1, 1));
        assert_eq!(restored.problems, Vec::<String>::new());
        assert_eq!(restored.schema_version, SCHEMA_VERSION);
        // the restored data knows it was backed up when the file was made
        assert_eq!(
            store.last_backup_at().unwrap().as_deref(),
            Some(restored.created_at.as_str())
        );
        // the hashes in the database still hold
        let files = store.original_files().unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(
            originals::problems(data.path(), &files),
            Vec::<String>::new()
        );
        // nothing was left lying around in the data folder
        let mut names: Vec<_> = std::fs::read_dir(data.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|name| !name.starts_with("e-rng.sqlite3"))
            .collect();
        names.sort();
        assert_eq!(names, ["backups", "invoices", "layouts", "offers"]);
    }

    #[test]
    fn the_file_is_a_zip_with_a_manifest_that_names_every_file() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());

        let archive = backup_of(&store, data.path(), out.path());

        let manifest = manifest_of(&archive);
        assert_eq!(manifest.format, "e-rng-backup");
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
        assert_eq!(manifest.app_version, APP);
        let paths: Vec<_> = manifest.files.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "e-rng.sqlite3",
                "invoices/1/RE-2026-001.pdf",
                "invoices/1/RE-2026-001.xml",
                "offers/1/AN-2026-001.pdf",
                "layouts/fonts/Sans-Regular.ttf",
                "layouts/images/logo.png",
            ]
        );
        let pdf = manifest
            .files
            .iter()
            .find(|e| e.path == "invoices/1/RE-2026-001.pdf")
            .unwrap();
        assert_eq!(pdf.size, b"%PDF invoice".len() as u64);
        assert_eq!(pdf.sha256, format!("{:x}", Sha256::digest(b"%PDF invoice")));
        // nothing else is in the archive, and the live connection is still the one in use
        assert_eq!(
            ZipArchive::new(File::open(&archive).unwrap())
                .unwrap()
                .len(),
            manifest.files.len() + 1
        );
        assert_eq!(store.counts().unwrap(), counts(1, 1, 1, 1));
    }

    #[test]
    fn a_failed_write_leaves_no_file_behind() {
        let out = tempfile::tempdir().unwrap();
        let target = out.path().join("never.erng-backup");
        let sources = vec![Source {
            name: "invoices/1/gone.pdf".into(),
            path: out.path().join("not-there"),
        }];

        assert!(write_archive(&target, &sources, 11, APP, "now").is_err());

        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_tampered_file_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let archive = backup_of(&store, data.path(), out.path());
        let tampered = out.path().join("tampered.erng-backup");
        rewritten(&archive, &tampered, |name, content| {
            Some(if name == "invoices/1/RE-2026-001.pdf" {
                b"%PDF something else".to_vec()
            } else {
                content
            })
        });

        let message = refusal(&store, data.path(), &tampered);

        assert!(message.contains("invoices/1/RE-2026-001.pdf"), "{message}");
        assert!(message.contains("Prüfsumme"), "{message}");
    }

    #[test]
    fn a_file_that_is_not_in_the_manifest_or_missing_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let archive = backup_of(&store, data.path(), out.path());

        let missing = out.path().join("missing.erng-backup");
        rewritten(&archive, &missing, |name, content| {
            (name != "offers/1/AN-2026-001.pdf").then_some(content)
        });
        assert!(refusal(&store, data.path(), &missing).contains("fehlt"));

        let added = out.path().join("added.erng-backup");
        std::fs::copy(&archive, &added).unwrap();
        let mut zip = ZipWriter::new_append(
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&added)
                .unwrap(),
        )
        .unwrap();
        zip.start_file("layouts/images/extra.png", options())
            .unwrap();
        zip.write_all(b"extra").unwrap();
        zip.finish().unwrap();
        assert!(refusal(&store, data.path(), &added).contains("extra.png"));
    }

    #[test]
    fn a_name_that_leads_out_of_the_folder_is_refused() {
        assert!(valid_name("e-rng.sqlite3"));
        assert!(valid_name("invoices/1/RE-1.pdf"));
        assert!(valid_name("layouts/fonts/Sans.ttf"));
        for bad in [
            "../evil",
            "invoices/../../evil",
            "/etc/passwd",
            "invoices",
            "invoices/",
            "invoices//a.pdf",
            "other/a.pdf",
            "invoices\\a.pdf",
            "backups/a.erng-backup",
            "invoices/./a.pdf",
            "",
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn a_file_without_manifest_or_no_zip_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let archive = backup_of(&store, data.path(), out.path());

        let bare = out.path().join("bare.erng-backup");
        rewritten(&archive, &bare, |name, content| {
            (name != MANIFEST).then_some(content)
        });
        let message = refusal(&store, data.path(), &bare);
        assert!(message.contains("manifest.json"), "{message}");

        let text = out.path().join("text.erng-backup");
        std::fs::write(&text, "no archive at all").unwrap();
        let message = refusal(&store, data.path(), &text);
        assert!(message.contains("kein ZIP"), "{message}");

        let empty = out.path().join("empty.erng-backup");
        std::fs::write(&empty, "").unwrap();
        assert!(refusal(&store, data.path(), &empty).contains("keine Datensicherung"));

        let missing = out.path().join("nothing.erng-backup");
        assert!(refusal(&store, data.path(), &missing).contains("cannot open"));
    }

    #[test]
    fn a_manifest_of_another_format_or_a_newer_file_layout_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let archive = backup_of(&store, data.path(), out.path());
        let with = |version: Option<u64>, format: &str| {
            let target = out.path().join(format!("{format}-{version:?}.erng-backup"));
            rewritten(&archive, &target, |name, content| {
                if name != MANIFEST {
                    return Some(content);
                }
                let mut json: serde_json::Value = serde_json::from_slice(&content).unwrap();
                json["format"] = format.into();
                if let Some(version) = version {
                    json["version"] = version.into();
                }
                Some(serde_json::to_vec(&json).unwrap())
            });
            target
        };

        let message = refusal(&store, data.path(), &with(None, "something-else"));
        assert_eq!(message, "Das ist keine Datensicherung von e-rng.");
        let message = refusal(&store, data.path(), &with(Some(2), FORMAT));
        assert_eq!(message, NEWER_VERSION);
    }

    /// A database file at a schema, with what that version had, as a backup holds it
    fn archive_of_database(
        database: &Path,
        schema_version: i32,
        out: &Path,
        files: &[(&str, &[u8])],
    ) -> PathBuf {
        let work = tempfile::tempdir().unwrap();
        let mut sources = vec![Source {
            name: DB_FILE.into(),
            path: database.to_path_buf(),
        }];
        sources.extend(files.iter().map(|(name, content)| {
            let path = work.path().join(name.replace('/', "_"));
            std::fs::write(&path, content).unwrap();
            Source {
                name: (*name).into(),
                path,
            }
        }));
        let target = out.join("crafted.erng-backup");
        write_archive(
            &target,
            &sources,
            schema_version,
            APP,
            "2020-01-01T00:00:00Z",
        )
        .unwrap();
        target
    }

    /// A database as an earlier version left it: stamped back to `version`, without what came later
    fn old_database(path: &Path, version: i32) {
        let store = Store::open(path).unwrap();
        store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap();
        drop(store);
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(&format!(
            "DROP TABLE rates; DROP TABLE app_state; PRAGMA user_version = {version};"
        ))
        .unwrap();
    }

    #[test]
    fn a_backup_of_a_newer_schema_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let database = out.path().join("newer.sqlite3");
        store.snapshot_to(&database).unwrap();
        Connection::open(&database)
            .unwrap()
            .execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION + 1))
            .unwrap();
        let archive = archive_of_database(&database, SCHEMA_VERSION + 1, out.path(), &[]);

        let message = refusal(&store, data.path(), &archive);

        assert_eq!(
            message,
            "Diese Sicherung stammt aus einer neueren Version von e-rng. Bitte zuerst die App aktualisieren."
        );
    }

    #[test]
    fn a_database_that_fails_its_integrity_check_or_is_none_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());

        let not_database = out.path().join("text.sqlite3");
        std::fs::write(
            &not_database,
            "this is not sqlite, but long enough to look like a file at all",
        )
        .unwrap();
        let archive = archive_of_database(&not_database, SCHEMA_VERSION, out.path(), &[]);
        assert!(refusal(&store, data.path(), &archive).contains("Datenbank"));

        // a page of the file is overwritten: the hash is of the file as it is, only SQLite notices
        let broken = out.path().join("broken.sqlite3");
        store.snapshot_to(&broken).unwrap();
        let mut content = bytes(&broken);
        content[4096..8192].fill(0xAB);
        std::fs::write(&broken, content).unwrap();
        let archive = archive_of_database(&broken, SCHEMA_VERSION, out.path(), &[]);
        let message = refusal(&store, data.path(), &archive);
        assert!(message.contains("beschädigt"), "{message}");
    }

    #[test]
    fn a_backup_of_an_older_schema_is_migrated_on_the_way_in() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let database = out.path().join("old.sqlite3");
        old_database(&database, 10);
        let archive = archive_of_database(
            &database,
            10,
            out.path(),
            &[("invoices/1/old.pdf", b"%PDF from before")],
        );

        let restored = restore(&store, data.path(), &archive, APP).unwrap();

        assert_eq!(restored.schema_version, 10);
        assert_eq!(restored.counts, counts(1, 0, 0, 0));
        // the live connection is the migrated one: the table of schema 11 is there and filled
        assert_eq!(store.rates(1).unwrap().len(), 2);
        assert_eq!(store.last_backup_at().unwrap(), None);
        assert_eq!(
            bytes(&data.path().join("invoices/1/old.pdf")),
            b"%PDF from before"
        );
        // what was there before is not: replaced as a whole
        assert!(!data.path().join("layouts").exists());
        assert!(!data.path().join("offers").exists());
        // and the app can use the data
        store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap();
    }

    #[test]
    fn a_database_that_cannot_be_migrated_is_refused_before_anything_changes() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let database = out.path().join("hollow.sqlite3");
        // old enough to need the step that reads the contexts, which are not there
        Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE unrelated (x); PRAGMA user_version = 10;")
            .unwrap();
        let archive = archive_of_database(&database, 10, out.path(), &[]);

        let message = refusal(&store, data.path(), &archive);

        assert!(
            message.contains("auf den Stand dieser Version"),
            "{message}"
        );
        assert_eq!(store.counts().unwrap(), counts(1, 1, 1, 1));
    }

    #[test]
    fn a_failure_in_the_middle_of_a_swap_leaves_the_old_data() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let before = kept(data.path());
        // what a restore would bring
        let other = tempfile::tempdir().unwrap();
        let _ = filled(other.path());
        write(other.path(), "layouts/images/other.png", b"other");
        let archive = backup_of(
            &Store::open(&other.path().join(DB_FILE)).unwrap(),
            other.path(),
            out.path(),
        );
        let staging = staging_dir(data.path()).unwrap();
        prepare(&archive, staging.path()).unwrap();
        // the database and the first folders move aside, then one cannot: the place is taken
        let aside = data.path().join(".erng-replaced-test");
        write(&aside, "offers/blocker/file", b"x");

        let error = replace_data(&store, data.path(), staging.path(), &aside).unwrap_err();

        assert!(format!("{error:#}").contains("unverändert"), "{error:#}");
        assert_eq!(kept(data.path()), before);
        // the app still has its data, from the database file that was put back
        assert_eq!(store.counts().unwrap(), counts(1, 1, 1, 1));
        assert_eq!(store.original_files().unwrap().len(), 3);
        store
            .save_draft(None, 1, Kind::Invoice, &draft(), None)
            .unwrap();
        // and the staged data is untouched, still there to try again
        assert!(staging.path().join(DB_FILE).is_file());
        assert!(staging.path().join("layouts/images/other.png").is_file());
        assert!(!aside.join(DB_FILE).exists());
    }

    #[test]
    fn a_new_database_that_does_not_open_puts_the_old_one_back() {
        let data = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let before = kept(data.path());
        // staged data whose database is not one (only a restore of a checked file cannot do this)
        let staging = staging_dir(data.path()).unwrap();
        std::fs::write(
            staging.path().join(DB_FILE),
            "garbage that is no database at all",
        )
        .unwrap();
        let aside = aside_dir(data.path());

        let error = replace_data(&store, data.path(), staging.path(), &aside).unwrap_err();

        assert!(format!("{error:#}").contains("does not open"), "{error:#}");
        assert_eq!(kept(data.path()), before);
        assert_eq!(store.counts().unwrap(), counts(1, 1, 1, 1));
        assert!(!aside.exists());
    }

    #[test]
    fn the_data_in_use_is_backed_up_before_it_is_replaced() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let archive = backup_of(&store, data.path(), out.path());
        let before = kept(data.path());
        write(
            data.path(),
            "invoices/1/newer.pdf",
            b"made after the backup",
        );

        let restored = restore(&store, data.path(), &archive, APP).unwrap();

        // the safety copy is in the data folder, holds what was there, and is a backup like any
        assert!(restored
            .safety_copy
            .starts_with(data.path().join("backups")));
        let name = restored.safety_copy.file_name().unwrap().to_string_lossy();
        assert!(
            name.ends_with("-vor-wiederherstellung.erng-backup"),
            "{name}"
        );
        assert!(name.ends_with(".erng-backup"), "{name}");
        let manifest = manifest_of(&restored.safety_copy);
        assert!(manifest
            .files
            .iter()
            .any(|e| e.path == "invoices/1/newer.pdf"));
        // the file made after the backup is gone from the data in use
        assert_eq!(kept(data.path()), before);
        // and the safety copy can bring it back
        restore(&store, data.path(), &restored.safety_copy, APP).unwrap();
        assert!(data.path().join("invoices/1/newer.pdf").is_file());
    }

    #[test]
    fn a_safety_backup_is_not_the_last_backup() {
        let data = tempfile::tempdir().unwrap();
        let store = filled(data.path());

        remove_all(&store, data.path(), APP).unwrap();

        assert_eq!(store.last_backup_at().unwrap(), None);
    }

    #[test]
    fn only_the_newest_three_safety_backups_of_a_kind_stay() {
        let data = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        let dir = safety_dir(data.path());
        std::fs::create_dir_all(&dir).unwrap();
        let old = [
            "2020-01-01-000000-vor-wiederherstellung.erng-backup",
            "2020-01-02-000000-vor-wiederherstellung.erng-backup",
            "2020-01-03-000000-vor-wiederherstellung.erng-backup",
            "2019-01-01-000000-vor-entfernen.erng-backup",
            "mine.erng-backup",
        ];
        old.iter()
            .for_each(|name| std::fs::write(dir.join(name), b"x").unwrap());

        let made = safety_backup(&store, data.path(), Reason::Restore, APP).unwrap();

        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "2019-01-01-000000-vor-entfernen.erng-backup".to_owned(),
                "2020-01-02-000000-vor-wiederherstellung.erng-backup".to_owned(),
                "2020-01-03-000000-vor-wiederherstellung.erng-backup".to_owned(),
                made.file_name().unwrap().to_string_lossy().into_owned(),
                "mine.erng-backup".to_owned(),
            ]
        );
    }

    #[test]
    fn removing_everything_leaves_a_first_start() {
        let data = tempfile::tempdir().unwrap();
        let store = filled(data.path());

        let removed = remove_all(&store, data.path(), APP).unwrap();

        assert_eq!(store.counts().unwrap(), counts(0, 0, 0, 0));
        assert_eq!(store.contexts().unwrap().len(), 0);
        assert_eq!(store.original_files().unwrap().len(), 0);
        for folder in FOLDERS {
            assert!(!data.path().join(folder).exists(), "{folder}");
        }
        // the database is a fresh one at the current schema, the safety copy is the old data
        assert_eq!(
            schema_of(&data.path().join(DB_FILE)).unwrap(),
            SCHEMA_VERSION
        );
        let manifest = manifest_of(&removed.safety_copy);
        assert_eq!(manifest.files.len(), 6);
        assert!(removed
            .safety_copy
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-vor-entfernen.erng-backup"));
        // and it works as a first start does
        let id = store
            .save_context(&Context::new(settings(), ""), None, None)
            .unwrap()
            .id;
        assert_eq!(id, Some(1));
    }

    #[test]
    fn changed_originals_are_reported_after_a_restore() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let store = filled(data.path());
        // a backup in which the database knows a file that is not what it was, as from a disk that
        // lost it before the backup was made: the manifest is true to the bytes, the database is not
        std::fs::write(
            data.path().join("invoices/1/RE-2026-001.xml"),
            b"<changed/>",
        )
        .unwrap();
        let archive = backup_of(&store, data.path(), out.path());

        let restored = restore(&store, data.path(), &archive, APP).unwrap();

        assert_eq!(restored.problems.len(), 1);
        assert!(restored.problems[0].contains("RE-2026-001.xml"));
        assert!(restored.problems[0].contains("verändert"));
    }

    #[test]
    fn leftovers_of_a_cut_off_run_are_cleared() {
        let data = tempfile::tempdir().unwrap();
        write(data.path(), ".erng-restore-abc/e-rng.sqlite3", b"x");
        write(data.path(), ".erng-backup-abc/e-rng.sqlite3", b"x");
        write(
            data.path(),
            ".erng-replaced-1/e-rng.sqlite3",
            b"the only copy",
        );
        write(data.path(), "invoices/1/a.pdf", b"x");

        clean_leftovers(data.path());

        assert!(!data.path().join(".erng-restore-abc").exists());
        assert!(!data.path().join(".erng-backup-abc").exists());
        assert!(data.path().join(".erng-replaced-1").exists());
        assert!(data.path().join("invoices/1/a.pdf").exists());
    }

    #[test]
    fn the_time_of_the_last_backup_is_kept_in_the_database() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.last_backup_at().unwrap(), None);

        store.set_last_backup_at("2026-10-29T10:00:00Z").unwrap();
        store.set_last_backup_at("2026-10-30T10:00:00Z").unwrap();

        assert_eq!(
            store.last_backup_at().unwrap().as_deref(),
            Some("2026-10-30T10:00:00Z")
        );
        // a database that has schema 11 from before the table existed gets it when it is opened
        let data = tempfile::tempdir().unwrap();
        let path = data.path().join(DB_FILE);
        drop(Store::open(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .execute_batch("DROP TABLE app_state;")
            .unwrap();
        assert_eq!(Store::open(&path).unwrap().last_backup_at().unwrap(), None);
    }
}
