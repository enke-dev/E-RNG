//! What a user's own layout brings along: fonts and pictures. Both are copied into the
//! app's data directory when picked, so the layout keeps working when the original is moved.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::{bail, Context, Result};
use ttf_parser::name_id;

use crate::layout::{CustomLayout, FontFile, ImageFile, PartContent};

const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];
const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "svg"];

/// A picture bigger than this is no logo, and would make every invoice heavy.
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// A short name that changes with the content, so a changed file never replaces one that an
/// exported invoice still refers to.
fn fingerprint(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Copies a picture for a part and reads how many pixels it has.
pub fn import_image(source: &Path, dir: &Path) -> Result<ImageFile> {
    let size = std::fs::metadata(source)
        .with_context(|| format!("cannot read the file {}", source.display()))?
        .len();
    // refuse before reading it all
    if size > MAX_IMAGE_BYTES {
        bail!(too_big(size));
    }
    let name = source
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    store_image(&name, &std::fs::read(source)?, dir)
}

fn too_big(size: u64) -> String {
    format!(
        "the image has {} MB, up to {} MB are accepted",
        size / (1024 * 1024) + 1,
        MAX_IMAGE_BYTES / (1024 * 1024)
    )
}

/// Keeps picture data under a name that changes with the content, the name says the type.
pub fn store_image(name: &str, bytes: &[u8], dir: &Path) -> Result<ImageFile> {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
        .context("an image must be a png, jpg or svg file")?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        bail!(too_big(bytes.len() as u64));
    }
    let vector = extension == "svg";
    let (width_px, height_px) = if vector {
        (0, 0)
    } else {
        let dimensions = imagesize::blob_size(bytes).context("this file is not a usable image")?;
        (
            u32::try_from(dimensions.width).unwrap_or(u32::MAX),
            u32::try_from(dimensions.height).unwrap_or(u32::MAX),
        )
    };

    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("image-{}.{extension}", fingerprint(bytes)));
    std::fs::write(&path, bytes)?;
    Ok(ImageFile {
        name: name.to_string(),
        path,
        width_px,
        height_px,
        vector,
    })
}

/// Copies the font and reads the family name Typst will know it by.
pub fn import_font(source: &Path, dir: &Path) -> Result<FontFile> {
    let bytes = std::fs::read(source)
        .with_context(|| format!("cannot read the file {}", source.display()))?;
    store_font(&source.to_string_lossy(), &bytes, dir)
}

/// Keeps font data under a name that changes with the content; `name` only tells the type.
pub fn store_font(name: &str, bytes: &[u8], dir: &Path) -> Result<FontFile> {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| FONT_EXTENSIONS.contains(&e.as_str()))
        .context("a font must be a ttf, otf, ttc or otc file")?;
    let family = family_name(bytes).context("this file is not a usable font")?;
    let style = style_name(bytes).unwrap_or_default();

    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.{extension}", fingerprint(bytes)));
    std::fs::write(&path, bytes)?;
    Ok(FontFile {
        family,
        style,
        path,
    })
}

/// The typographic family if the font has one (it groups weights better), else the plain family.
fn family_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    [name_id::TYPOGRAPHIC_FAMILY, name_id::FAMILY]
        .iter()
        .find_map(|id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == *id && n.is_unicode())
                .find_map(|n| n.to_string())
        })
}

/// The weight and style the font calls itself (`Regular`, `Bold Italic`), a variable font says what
/// its default is.
fn style_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    [name_id::TYPOGRAPHIC_SUBFAMILY, name_id::SUBFAMILY]
        .iter()
        .find_map(|id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == *id && n.is_unicode())
                .find_map(|n| n.to_string())
        })
}

/// The style of a font that is kept already, for fonts that were stored before it was read
pub fn style_of(path: &Path) -> Option<String> {
    style_name(&std::fs::read(path).ok()?)
}

/// Removes the picture and font files no layout points at any more: those of deleted layouts and
/// of files that were replaced. Only files this module made are touched, and only when nothing is
/// being edited (at start), a freshly picked file is not in any layout yet.
pub fn prune(dir: &Path, layouts: &[CustomLayout]) -> Result<usize> {
    let images = layouts.iter().flat_map(|layout| {
        let setups = std::iter::once(&layout.page).chain(layout.first.as_ref());
        setups
            .flat_map(|setup| setup.parts.iter())
            .filter_map(|part| match &part.content {
                PartContent::Image { image: Some(image) } => Some(image.path.as_path()),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    let used: HashSet<&Path> = layouts
        .iter()
        .flat_map(|layout| layout.fonts.iter().map(|f| f.path.as_path()))
        .chain(images)
        .collect();
    let removable = |path: &Path| {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let in_fonts = path.parent().is_some_and(|p| p.ends_with("fonts"));
        let is = |extensions: &[&str]| extensions.iter().any(|e| name.ends_with(&format!(".{e}")));
        (name.starts_with("background-") && name.ends_with(".svg"))
            || (name.starts_with("image-") && is(&IMAGE_EXTENSIONS))
            || (in_fonts && is(&FONT_EXTENSIONS))
    };
    let mut removed = 0;
    for folder in [dir.to_path_buf(), dir.join("fonts")] {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if path.is_file() && removable(&path) && !used.contains(path.as_path()) {
                std::fs::remove_file(&path)?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-fixtures")
            .join(name)
    }

    #[test]
    fn the_family_of_a_font_is_read_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let font = import_font(&bundled("OpenSans.ttf"), dir.path()).unwrap();
        assert_eq!(font.family, "Open Sans");
        assert_eq!(font.style, "Regular");
        assert!(font.path.starts_with(dir.path()));
    }

    #[test]
    fn files_no_layout_points_at_are_pruned_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let kept = import_font(&bundled("OpenSans.ttf"), &dir.path().join("fonts")).unwrap();
        let stray = dir.path().join("fonts/0000.ttf");
        let old_page = dir.path().join("background-abc-1.svg");
        let foreign = dir.path().join("notes.txt");
        for file in [&stray, &old_page, &foreign] {
            std::fs::write(file, "x").unwrap();
        }
        let layout = CustomLayout {
            id: Some(1),
            name: "L".into(),
            fonts: vec![kept.clone()],
            ..crate::layout::builtin()
        };

        assert_eq!(prune(dir.path(), &[layout]).unwrap(), 2);

        assert!(kept.path.exists() && foreign.exists());
        assert!(!stray.exists() && !old_page.exists());
    }

    #[test]
    fn an_image_is_copied_measured_and_capped() {
        let dir = tempfile::tempdir().unwrap();
        // the header of a png: 8 bytes of signature, then the IHDR chunk with 300 x 200 pixels
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&300u32.to_be_bytes());
        png.extend_from_slice(&200u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        let source = dir.path().join("logo.png");
        std::fs::write(&source, &png).unwrap();

        let image = import_image(&source, &dir.path().join("kept")).unwrap();
        assert_eq!(
            (image.width_px, image.height_px, image.vector),
            (300, 200, false)
        );
        assert!(image.path.exists());

        let svg = dir.path().join("logo.svg");
        std::fs::write(&svg, "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        assert!(import_image(&svg, &dir.path().join("kept")).unwrap().vector);

        let big = dir.path().join("big.jpg");
        std::fs::write(
            &big,
            vec![0u8; usize::try_from(MAX_IMAGE_BYTES).unwrap() + 1],
        )
        .unwrap();
        assert!(import_image(&big, &dir.path().join("kept")).is_err());
        assert!(import_image(&dir.path().join("note.txt"), &dir.path().join("kept")).is_err());
    }

    #[test]
    fn other_files_are_not_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("note.ttf");
        std::fs::write(&text, "no font").unwrap();
        assert!(import_font(&text, dir.path()).is_err());
        assert!(import_font(&dir.path().join("a.txt"), dir.path()).is_err());
    }
}
