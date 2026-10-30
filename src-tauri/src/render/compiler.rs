//! The Typst compiler linked into the app, at the version the output was verified with: a world
//! that serves the folder of one render, the fonts Typst brings itself and the ones of an own
//! layout, and the exports the render needs. There are no system fonts and no packages, so the
//! output does not depend on the machine it was made on.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Local, TimeZone, Timelike};
use typst::diag::{FileError, FileResult, Severity, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Duration, Label, Selector};
use typst::introspection::{Introspector, MetadataElem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::{LazyHash, PicoStr};
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{self, FontStore};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};
use typst_svg::SvgOptions;

/// Typst's standard library, built once
static LIBRARY: LazyLock<LazyHash<Library>> =
    LazyLock::new(|| LazyHash::new(Library::builder().build()));

/// The fonts Typst brings itself (the same the CLI had), parsed once
static EMBEDDED: LazyLock<Vec<(Font, FontInfo)>> = LazyLock::new(|| fonts::embedded().collect());

/// The font book and the loaded fonts of the embedded fonts alone, what a render without fonts
/// of its own shares
static SHARED: LazyLock<FontStore> = LazyLock::new(|| {
    let mut store = FontStore::new();
    store.extend(EMBEDDED.iter().cloned());
    store
});

/// How many compilations a cached result of the compiler survives unused
const CACHE_AGE: usize = 10;

/// The fonts of one render: the shared embedded ones, or those plus the layout's own
enum Fonts {
    Shared(&'static FontStore),
    Own(FontStore),
}

impl Fonts {
    /// The embedded fonts, then the ones in `dir`: the order the CLI had them in
    fn of(dir: &Path) -> Self {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Self::Shared(&SHARED);
        };
        let mut files: Vec<_> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.is_file())
            .collect();
        // the directory has no order of its own, the font a family falls back to must not change
        files.sort();

        let mut store = FontStore::new();
        store.extend(EMBEDDED.iter().cloned());
        store.extend(
            files
                .iter()
                .filter_map(|path| std::fs::read(path).ok())
                .flat_map(|data| Font::iter(Bytes::new(data)))
                .map(|font| {
                    let info = font.info().clone();
                    (font, info)
                }),
        );
        Self::Own(store)
    }

    fn store(&self) -> &FontStore {
        match self {
            Self::Shared(store) => store,
            Self::Own(store) => store,
        }
    }
}

/// Serves the files of the project folder, a package is never found
struct Project(FsRoot);

impl FileLoader for Project {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        match id.root() {
            VirtualRoot::Project => self.0.load(id.vpath()),
            VirtualRoot::Package(spec) => Err(FileError::Other(Some(
                format!("packages are not available ({spec})").into(),
            ))),
        }
    }
}

/// The creation time of the document if `SOURCE_DATE_EPOCH` fixes it, so that two renders of one
/// document compare equal
fn fixed_time() -> Option<i64> {
    std::env::var("SOURCE_DATE_EPOCH").ok()?.trim().parse().ok()
}

/// What one compilation sees: the folder as the root, `main` as the file to compile
struct RenderWorld {
    fonts: Fonts,
    files: FileStore<Project>,
    main: FileId,
    now: Time,
}

impl RenderWorld {
    fn new(dir: &Path, main: &str) -> Result<Self> {
        let root = dir
            .canonicalize()
            .with_context(|| format!("cannot read {}", dir.display()))?;
        let main = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new(main).map_err(|e| anyhow!("{main}: {e:?}"))?,
        )
        .intern();
        let now = match fixed_time() {
            Some(seconds) => Time::fixed_timestamp(seconds).map_err(|e| anyhow!("{e}"))?,
            None => Time::system(),
        };
        Ok(Self {
            fonts: Fonts::of(&root.join(super::FONTS_DIR)),
            files: FileStore::new(Project(FsRoot::new(root))),
            main,
            now,
        })
    }
}

impl World for RenderWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.store().book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.store().font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}

/// The diagnostics as lines of text, each with its place in the source
fn describe(world: &RenderWorld, diagnostics: &[SourceDiagnostic]) -> String {
    let mut text = String::new();
    diagnostics.iter().for_each(|diagnostic| {
        let place = diagnostic.span.id().and_then(|id| {
            let source = world.source(id).ok()?;
            let line = source
                .lines()
                .byte_to_line(world.range(diagnostic.span)?.start)?;
            Some(format!("{}:{}: ", id.vpath().get_without_slash(), line + 1))
        });
        let kind = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let _ = writeln!(
            text,
            "{kind}: {}{}",
            place.unwrap_or_default(),
            diagnostic.message
        );
        diagnostic
            .hints
            .iter()
            .filter(|hint| hint.span.is_detached())
            .for_each(|hint| {
                let _ = writeln!(text, "hint: {}", hint.v);
            });
    });
    text
}

/// Compiles the file `main` of `dir` (the root of the project, the fonts of an own layout in its
/// `fonts` folder). The warnings of a good compilation are dropped, a failed one reports its
/// errors followed by them.
pub(super) fn compile(dir: &Path, main: &str) -> Result<PagedDocument> {
    let world = RenderWorld::new(dir, main)?;
    let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
    // the compiler memoizes what it computes, without this the cache only grows
    typst::comemo::evict(CACHE_AGE);
    output.map_err(|mut errors| {
        errors.extend(warnings);
        anyhow!("typst failed: {}", describe(&world, &errors).trim())
    })
}

/// A moment as the date and time Typst knows
fn convert<Tz: TimeZone>(time: &chrono::DateTime<Tz>) -> Option<Datetime> {
    Datetime::from_ymd_hms(
        time.year(),
        time.month().try_into().ok()?,
        time.day().try_into().ok()?,
        time.hour().try_into().ok()?,
        time.minute().try_into().ok()?,
        time.second().try_into().ok()?,
    )
}

/// The time stamp of a PDF: `SOURCE_DATE_EPOCH` in UTC, else now in the local time zone
fn timestamp() -> Option<Timestamp> {
    if let Some(seconds) = fixed_time() {
        chrono::DateTime::from_timestamp(seconds, 0)
            .and_then(|time| convert(&time))
            .map(Timestamp::new_utc)
    } else {
        let now = Local::now();
        convert(&now)
            .and_then(|time| Timestamp::new_local(time, now.offset().local_minus_utc() / 60))
    }
}

/// The document as a PDF/A-3b (what `--pdf-standard a-3b` did)
pub(super) fn pdf(document: &PagedDocument) -> Result<Vec<u8>> {
    let options = PdfOptions {
        standards: PdfStandards::new(&[PdfStandard::A_3b])
            .map_err(|e| anyhow!("PDF/A-3b is not available: {}", e.message()))?,
        timestamp: timestamp(),
        ..PdfOptions::default()
    };
    typst_pdf::pdf(document, &options).map_err(|errors| {
        let messages: Vec<_> = errors.iter().map(|e| e.message.to_string()).collect();
        anyhow!("typst failed: {}", messages.join("; "))
    })
}

/// One SVG per page
pub(super) fn svg(document: &PagedDocument) -> Vec<String> {
    let options = SvgOptions {
        render_bleed: false,
        pretty: false,
    };
    document
        .pages()
        .iter()
        .map(|page| typst_svg::svg(page, &options))
        .collect()
}

/// The values of the `metadata` elements with the label `label`, in the order of the document
pub(super) fn metadata(document: &PagedDocument, label: &str) -> Result<Vec<serde_json::Value>> {
    let Some(label) = Label::new(PicoStr::intern(label)) else {
        bail!("an empty label");
    };
    document
        .introspector()
        .query(&Selector::Label(label))
        .iter()
        .map(|content| {
            let element = content
                .to_packed::<MetadataElem>()
                .context("the element is no metadata")?;
            serde_json::to_value(&element.value).map_err(Into::into)
        })
        .collect()
}

/// A page as a PNG at `ppi`, for looking at the SVG of the tests
#[cfg(test)]
pub(super) fn png(document: &PagedDocument, ppi: f64) -> Result<Vec<u8>> {
    let options = typst_render::RenderOptions {
        pixel_per_pt: typst::utils::Scalar::new(ppi / 72.0),
        render_bleed: false,
    };
    let page = document
        .pages()
        .first()
        .context("a document without pages")?;
    typst_render::render(page, &options)
        .encode_png()
        .map_err(|e| anyhow!("cannot encode the PNG: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_file_reports_where() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.typ"), "= Title\n#nothing-here\n").unwrap();

        let error = compile(dir.path(), "a.typ").unwrap_err().to_string();

        assert!(
            error.starts_with("typst failed: error: a.typ:2:"),
            "{error}"
        );
    }

    #[test]
    fn a_package_is_never_loaded() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.typ"),
            "#import \"@preview/example:0.1.0\": x\n",
        )
        .unwrap();

        let error = compile(dir.path(), "a.typ").unwrap_err().to_string();

        assert!(error.contains("packages are not available"), "{error}");
    }

    #[test]
    fn the_fonts_are_the_embedded_ones() {
        let families: Vec<_> = SHARED
            .book()
            .families()
            .map(|(name, _)| name.to_string())
            .collect();

        assert!(
            families.iter().any(|f| f == "DejaVu Sans Mono"),
            "{families:?}"
        );
        assert!(
            families.iter().any(|f| f == "Libertinus Serif"),
            "{families:?}"
        );
        assert!(!families.iter().any(|f| f == "Menlo"), "{families:?}");
    }

    #[test]
    fn the_values_of_labelled_metadata_come_out_in_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.typ"),
            "#metadata((n: 1, w: 2.5)) <box>\n#pagebreak()\n#metadata((n: 2, w: 3.0)) <box>\n",
        )
        .unwrap();

        let document = compile(dir.path(), "a.typ").unwrap();
        let values = metadata(&document, "box").unwrap();

        assert_eq!(
            values,
            vec![
                serde_json::json!({"n": 1, "w": 2.5}),
                serde_json::json!({"n": 2, "w": 3.0}),
            ]
        );
    }
}
