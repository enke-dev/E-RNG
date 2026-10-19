//! A layout as a file: the layout itself with the fonts and pictures it uses, so it can be kept,
//! moved to another machine or handed to someone else. An imported file is untrusted: it is
//! checked, and every font and picture goes through the same intake as one that was picked.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::layout::{CustomLayout, ImageFile, PartContent};
use crate::layout_files::{store_font, store_image};

const FORMAT: &str = "e-rng-layout";
const VERSION: u32 = 1;

/// More than this is no layout, the pictures and fonts are the bulk of it.
pub const MAX_BUNDLE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    format: String,
    version: u32,
    layout: CustomLayout,
    /// Font and picture data (base64) by the name the layout refers to it by in here
    #[serde(default)]
    files: BTreeMap<String, String>,
    /// Families of fonts that were left out on purpose, so the importer can tell what is missing
    #[serde(default)]
    omitted_fonts: Vec<String>,
}

/// What an import brings: the layout, and the fonts it asks for but did not bring.
#[derive(Debug, Serialize)]
pub struct Imported {
    pub layout: CustomLayout,
    pub missing_fonts: Vec<String>,
}

fn images_mut(layout: &mut CustomLayout) -> Vec<&mut ImageFile> {
    let first = layout.first.as_mut();
    std::iter::once(&mut layout.page)
        .chain(first)
        .flat_map(|setup| setup.parts.iter_mut())
        .filter_map(|part| match &mut part.content {
            PartContent::Image { image: Some(image) } => Some(image),
            _ => None,
        })
        .collect()
}

/// Reads a file of the layout, only from the folder the app keeps them in.
fn read_own(path: &Path, own: &Path) -> Result<Vec<u8>> {
    let inside = path.canonicalize().ok().zip(own.canonicalize().ok());
    ensure!(
        inside.is_some_and(|(file, dir)| file.starts_with(dir)),
        "{} does not belong to the layout files",
        path.display()
    );
    std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))
}

fn extension(path: &Path) -> String {
    path.extension()
        .map_or_else(String::new, |e| e.to_string_lossy().into_owned())
}

/// The layout as the text of a file. `own` is where the app keeps the files of layouts.
pub fn export(layout: &CustomLayout, with_fonts: bool, own: &Path) -> Result<String> {
    let mut layout = layout.clone();
    layout.id = None;
    layout.archived = false;
    let mut files = BTreeMap::new();
    let mut omitted_fonts = Vec::new();

    // the layout refers to the data by a name made here, nothing of the user's folders leaks
    if with_fonts {
        for (n, font) in layout.fonts.iter_mut().enumerate() {
            let key = format!("font-{n}.{}", extension(&font.path));
            files.insert(key.clone(), STANDARD.encode(read_own(&font.path, own)?));
            font.path = PathBuf::from(key);
        }
    } else {
        omitted_fonts = layout.fonts.iter().map(|f| f.family.clone()).collect();
        omitted_fonts.dedup();
        layout.fonts.clear();
    }
    for (n, image) in images_mut(&mut layout).into_iter().enumerate() {
        let key = format!("image-{n}.{}", extension(&image.path));
        files.insert(key.clone(), STANDARD.encode(read_own(&image.path, own)?));
        image.path = PathBuf::from(key);
    }

    Ok(serde_json::to_string_pretty(&Bundle {
        format: FORMAT.into(),
        version: VERSION,
        layout,
        files,
        omitted_fonts,
    })?)
}

/// A layout file that has been read and found fit; nothing of it is on disk yet.
#[derive(Debug)]
pub struct Parsed(Bundle);

impl Parsed {
    pub fn name(&self) -> &str {
        &self.0.layout.name
    }

    /// Empty for a layout that was never stored (the built-in one)
    pub fn uid(&self) -> &str {
        &self.0.layout.uid
    }

    pub fn updated_at(&self) -> &str {
        &self.0.layout.updated_at
    }
}

/// Checks the text of a file, without keeping anything of it.
pub fn parse(text: &str) -> Result<Parsed> {
    let mut bundle: Bundle = serde_json::from_str(text).context("this is not a layout file")?;
    ensure!(bundle.format == FORMAT, "this is not a layout file");
    if bundle.version > VERSION {
        bail!("the layout file is from a newer version of the app");
    }
    bundle.layout.name = bundle.layout.name.trim().to_string();
    ensure!(!bundle.layout.name.is_empty(), "the layout has no name");
    Ok(Parsed(bundle))
}

/// Takes the layout and its files in: they are copied into `own` like picked ones. The layout is
/// not stored here, `name` replaces the one in the file.
pub fn install(parsed: Parsed, name: Option<&str>, own: &Path) -> Result<Imported> {
    let bundle = parsed.0;
    let mut layout = bundle.layout;
    layout.id = None;
    layout.archived = false;
    if let Some(name) = name {
        layout.name = name.trim().to_string();
    }

    let data = |key: &Path| -> Result<Vec<u8>> {
        let name = key.to_string_lossy();
        // a name from the file, never a place on disk
        ensure!(
            key.components().count() == 1
                && matches!(key.components().next(), Some(Component::Normal(_))),
            "the layout file refers to {name}"
        );
        let encoded = bundle
            .files
            .get(name.as_ref())
            .with_context(|| format!("the layout file lacks {name}"))?;
        STANDARD
            .decode(encoded)
            .with_context(|| format!("{name} is damaged"))
    };

    for font in &mut layout.fonts {
        let kept = store_font(
            &font.path.to_string_lossy(),
            &data(&font.path)?,
            &own.join("fonts"),
        )
        .context("a font of the layout file is not usable")?;
        *font = kept;
    }
    for image in images_mut(&mut layout) {
        let name = image.name.clone();
        *image = store_image(&image.path.to_string_lossy(), &data(&image.path)?, own)
            .context("a picture of the layout file is not usable")?;
        image.name = name;
    }

    Ok(Imported {
        layout,
        missing_fonts: bundle.omitted_fonts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{builtin, Anchor, Part};
    use crate::layout_files::import_font;

    fn font() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/OpenSans.ttf")
    }

    fn png() -> Vec<u8> {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&300u32.to_be_bytes());
        png.extend_from_slice(&200u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        png
    }

    /// A stored layout with a font and a picture in `own`
    fn stored(own: &Path) -> CustomLayout {
        let source = own.join("logo.png");
        std::fs::write(&source, png()).unwrap();
        let image = crate::layout_files::import_image(&source, own).unwrap();
        let mut layout = builtin();
        layout.id = Some(7);
        layout.name = "Mein Layout".into();
        layout.archived = true;
        layout.fonts = vec![import_font(&font(), &own.join("fonts")).unwrap()];
        layout.page.parts.push(Part {
            anchor: Anchor::TopRight,
            x: 10.0,
            y: 10.0,
            width: Some(40.0),
            content: PartContent::Image { image: Some(image) },
        });
        layout
    }

    #[test]
    fn a_layout_survives_the_trip_with_its_fonts_and_pictures() {
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let layout = stored(from.path());

        let text = export(&layout, true, from.path()).unwrap();
        assert!(!text.contains(&from.path().to_string_lossy().into_owned()));
        let imported = install(parse(&text).unwrap(), None, to.path()).unwrap();

        let got = imported.layout;
        assert_eq!((got.id, got.archived), (None, false));
        assert_eq!(got.name, layout.name);
        assert_eq!(got.page.margins, layout.page.margins);
        assert_eq!(got.page.parts.len(), layout.page.parts.len());
        assert_eq!(got.fonts[0].family, "Open Sans");
        assert!(got.fonts[0].path.starts_with(to.path()) && got.fonts[0].path.exists());
        let image = images_mut(&mut got.clone())
            .into_iter()
            .map(|i| (i.name.clone(), i.path.clone(), i.width_px))
            .next()
            .unwrap();
        assert_eq!((image.0.as_str(), image.2), ("logo.png", 300));
        assert!(image.1.starts_with(to.path()) && image.1.exists());
        assert!(imported.missing_fonts.is_empty());
    }

    #[test]
    fn fonts_can_stay_out_and_the_importer_hears_of_it() {
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let text = export(&stored(from.path()), false, from.path()).unwrap();

        assert!(!text.contains("font-0"));
        let imported = install(parse(&text).unwrap(), None, to.path()).unwrap();
        assert!(imported.layout.fonts.is_empty());
        assert_eq!(imported.missing_fonts, vec!["Open Sans".to_string()]);
    }

    #[test]
    fn files_from_elsewhere_are_not_exported() {
        let own = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let mut layout = stored(own.path());
        layout.fonts = vec![import_font(&font(), elsewhere.path()).unwrap()];

        assert!(export(&layout, true, own.path()).is_err());
    }

    #[test]
    fn a_damaged_or_foreign_file_is_refused() {
        let own = tempfile::tempdir().unwrap();
        let from = tempfile::tempdir().unwrap();
        let text = export(&stored(from.path()), true, from.path()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let with = |change: &dyn Fn(&mut serde_json::Value)| {
            let mut v = value.clone();
            change(&mut v);
            v.to_string()
        };

        // reading is two steps: the first needs no folder, the second copies the files
        let load = |text: &str| parse(text).and_then(|parsed| install(parsed, None, own.path()));
        assert!(load("no json").is_err());
        assert!(load(&with(&|v| v["format"] = "other".into())).is_err());
        assert!(load(&with(&|v| v["version"] = 99.into())).is_err());
        assert!(load(&with(&|v| v["layout"]["name"] = "  ".into())).is_err());
        assert!(load(&with(&|v| v["files"] = serde_json::json!({}))).is_err());
        // not a font, and a path that leaves the folder
        assert!(load(&with(
            &|v| v["files"]["font-0.ttf"] = STANDARD.encode("no font").into()
        ))
        .is_err());
        assert!(load(&with(
            &|v| v["layout"]["fonts"][0]["path"] = "../font-0.ttf".into()
        ))
        .is_err());
        assert!(load(&text).is_ok());
    }

    #[test]
    fn what_tells_copies_apart_travels_and_the_name_can_change() {
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let mut layout = stored(from.path());
        layout.uid = "u-1".into();
        layout.updated_at = "2026-10-20T08:00:00Z".into();
        let text = export(&layout, true, from.path()).unwrap();

        let parsed = parse(&text).unwrap();
        assert_eq!(
            (parsed.name(), parsed.uid(), parsed.updated_at()),
            ("Mein Layout", "u-1", "2026-10-20T08:00:00Z")
        );
        let imported = install(parsed, Some(" Anderer Name "), to.path()).unwrap();
        assert_eq!(imported.layout.name, "Anderer Name");
        assert_eq!(imported.layout.uid, "u-1");
    }
}
