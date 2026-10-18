//! What a user's own layout brings along: stationery (a PDF) and fonts. Both are copied into the
//! app's data directory when picked, so the layout keeps working when the original is moved.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;

use anyhow::{bail, Context, Result};
use hayro_interpret::InterpreterSettings;
use hayro_svg::hayro_syntax::Pdf;
use hayro_svg::{convert, RenderCache, SvgRenderSettings};
use ttf_parser::name_id;

use crate::model::{Background, CustomLayout, FontFile};

const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];

/// A short name that changes with the content, so a changed file never replaces one that an
/// exported invoice still refers to.
fn fingerprint(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// The first page of the PDF is the stationery of the first page, the second one (if there is
/// one) the stationery of all following pages.
pub fn import_background(source: &Path, dir: &Path) -> Result<Background> {
    let bytes = std::fs::read(source)
        .with_context(|| format!("cannot read the file {}", source.display()))?;
    let pdf = Pdf::new(std::sync::Arc::new(bytes.clone()))
        .map_err(|e| anyhow::anyhow!("not a usable PDF: {e:?}"))?;

    let cache = RenderCache::new();
    let interpreter = InterpreterSettings::default();
    let render = SvgRenderSettings::default();
    let key = fingerprint(&bytes);
    std::fs::create_dir_all(dir)?;

    let pages = pdf
        .pages()
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, page)| {
            let path = dir.join(format!("background-{key}-{}.svg", index + 1));
            std::fs::write(&path, convert(page, &cache, &interpreter, &render))?;
            Ok(path)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut pages = pages.into_iter();
    let Some(first) = pages.next() else {
        bail!("the PDF has no pages");
    };
    Ok(Background {
        name: source
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        first,
        rest: pages.next(),
    })
}

/// Copies the font and reads the family name Typst will know it by.
pub fn import_font(source: &Path, dir: &Path) -> Result<FontFile> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| FONT_EXTENSIONS.contains(&e.as_str()))
        .context("a font must be a ttf, otf, ttc or otc file")?;
    let bytes = std::fs::read(source)
        .with_context(|| format!("cannot read the file {}", source.display()))?;
    let family = family_name(&bytes).context("this file is not a usable font")?;

    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.{extension}", fingerprint(&bytes)));
    std::fs::write(&path, &bytes)?;
    Ok(FontFile { family, path })
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

/// Removes the stationery and font files no layout points at any more: those of deleted layouts and
/// of files that were replaced. Only files this module made are touched, and only when nothing is
/// being edited (at start), a freshly picked file is not in any layout yet.
pub fn prune(dir: &Path, layouts: &[CustomLayout]) -> Result<usize> {
    let used: HashSet<&Path> = layouts
        .iter()
        .flat_map(|layout| {
            let background = layout
                .background
                .iter()
                .flat_map(|b| std::iter::once(b.first.as_path()).chain(b.rest.as_deref()));
            background.chain(layout.fonts.iter().map(|f| f.path.as_path()))
        })
        .collect();
    let removable = |path: &Path| {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let in_fonts = path.parent().is_some_and(|p| p.ends_with("fonts"));
        (name.starts_with("background-") && name.ends_with(".svg"))
            || (in_fonts
                && FONT_EXTENSIONS
                    .iter()
                    .any(|e| name.ends_with(&format!(".{e}"))))
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
            background: None,
            margins: Default::default(),
            first_margins: None,
            fonts: vec![kept.clone()],
            archived: false,
        };

        assert_eq!(prune(dir.path(), &[layout]).unwrap(), 2);

        assert!(kept.path.exists() && foreign.exists());
        assert!(!stray.exists() && !old_page.exists());
    }

    #[test]
    fn other_files_are_not_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("note.ttf");
        std::fs::write(&text, "no font").unwrap();
        assert!(import_font(&text, dir.path()).is_err());
        assert!(import_font(&dir.path().join("a.txt"), dir.path()).is_err());
    }

    #[test]
    fn a_pdf_becomes_one_svg_per_page_up_to_two() {
        let dir = tempfile::tempdir().unwrap();
        assert!(import_background(&dir.path().join("missing.pdf"), dir.path()).is_err());
        let text = dir.path().join("fake.pdf");
        std::fs::write(&text, "no pdf").unwrap();
        assert!(import_background(&text, dir.path()).is_err());
    }
}
