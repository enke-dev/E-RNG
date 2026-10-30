//! What a user's own layout brings along: fonts and pictures. Both are copied into the
//! app's data directory when picked, so the layout keeps working when the original is moved.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;

use std::borrow::Cow;

use anyhow::{bail, ensure, Context, Result};
use ttf_parser::{name_id, PlatformId};

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
        .with_context(|| format!("Die Datei {} lässt sich nicht lesen.", source.display()))?
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
        "Das Bild hat {} MB, erlaubt sind bis zu {} MB.",
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
        .context("Ein Bild muss eine PNG-, JPG- oder SVG-Datei sein.")?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        bail!(too_big(bytes.len() as u64));
    }
    let vector = extension == "svg";
    let (width_px, height_px) = if vector {
        (0, 0)
    } else {
        let dimensions =
            imagesize::blob_size(bytes).context("Die Datei ist kein verwendbares Bild.")?;
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

/// Copies the font and reads the family name Typst will know it by. A collection (`ttc`, `otc`)
/// gives one font per face.
pub fn import_font(source: &Path, dir: &Path) -> Result<Vec<FontFile>> {
    let bytes = std::fs::read(source)
        .with_context(|| format!("Die Datei {} lässt sich nicht lesen.", source.display()))?;
    store_font(&source.to_string_lossy(), &bytes, dir)
}

/// More faces than this are no collection somebody means to use, and would multiply the data.
const MAX_FACES: u32 = 64;

/// Keeps font data under a name that changes with the content; `name` only tells the type. A
/// collection is split into one standalone file per face, so every face is a font of its own in
/// the layout (listed, removed and exported alone) and a single file stays as it is.
pub fn store_font(name: &str, bytes: &[u8], dir: &Path) -> Result<Vec<FontFile>> {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| FONT_EXTENSIONS.contains(&e.as_str()))
        .context("Eine Schrift muss eine TTF-, OTF-, TTC- oder OTC-Datei sein.")?;
    let faces: Vec<(Cow<[u8]>, String)> = match ttf_parser::fonts_in_collection(bytes) {
        Some(count) => {
            ensure!(
                count <= MAX_FACES,
                "Die Sammlung enthält zu viele Schriften."
            );
            (0..count)
                .map(|index| split_face(bytes, index))
                .collect::<Option<Vec<_>>>()
                .context("Die Datei ist keine verwendbare Schriftsammlung.")?
                .into_iter()
                .map(|(face, extension)| (Cow::Owned(face), extension))
                .collect()
        }
        None => vec![(Cow::Borrowed(bytes), extension)],
    };

    // a face without a readable name is left out, the others are fine
    let fonts = faces
        .iter()
        .filter_map(|(face, extension)| {
            let family = family_name(face)?;
            Some((
                face,
                extension,
                family,
                style_name(face).unwrap_or_default(),
            ))
        })
        .collect::<Vec<_>>();
    ensure!(
        !fonts.is_empty(),
        "Die Datei ist keine verwendbare Schrift."
    );

    std::fs::create_dir_all(dir)?;
    fonts
        .into_iter()
        .map(|(face, extension, family, style)| {
            let path = dir.join(format!("{}.{extension}", fingerprint(face)));
            std::fs::write(&path, face)?;
            Ok(FontFile {
                family,
                style,
                path,
            })
        })
        .collect()
}

fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The checksum of the OpenType tables: the sum of the data as big-endian 32 bit words, the last
/// one padded with zeros.
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// One font file from its tables: the directory (sorted by tag as the format asks), the tables
/// padded to four bytes, and the `head` adjustment that makes the checksum of the file come out.
fn assemble(version: [u8; 4], tables: &[([u8; 4], &[u8])]) -> Option<Vec<u8>> {
    let count = u16::try_from(tables.len()).ok()?;
    let mut tables: Vec<([u8; 4], Vec<u8>)> =
        tables.iter().map(|(t, d)| (*t, d.to_vec())).collect();
    tables.sort_by_key(|(tag, _)| *tag);
    // the sum of `head` is made with its adjustment at zero
    tables
        .iter_mut()
        .filter(|(tag, _)| tag == b"head")
        .try_for_each(|(_, data)| data.get_mut(8..12).map(|adjustment| adjustment.fill(0)))?;

    let entry_selector = count.checked_ilog2().unwrap_or(0);
    let search_range = (1u32 << entry_selector) * 16;
    let body_start = 12 + 16 * u32::from(count);
    let offsets: Vec<u32> = tables
        .iter()
        .scan(body_start, |next, (_, data)| {
            let at = *next;
            *next += u32::try_from(data.len().next_multiple_of(4)).unwrap_or(u32::MAX);
            Some(at)
        })
        .collect();

    let header = [
        version.to_vec(),
        count.to_be_bytes().to_vec(),
        u16::try_from(search_range).ok()?.to_be_bytes().to_vec(),
        u16::try_from(entry_selector).ok()?.to_be_bytes().to_vec(),
        u16::try_from(u32::from(count) * 16 - search_range)
            .ok()?
            .to_be_bytes()
            .to_vec(),
    ]
    .concat();
    let directory = tables
        .iter()
        .zip(&offsets)
        .flat_map(|((tag, data), offset)| {
            [
                tag.to_vec(),
                checksum(data).to_be_bytes().to_vec(),
                offset.to_be_bytes().to_vec(),
                u32::try_from(data.len())
                    .unwrap_or(u32::MAX)
                    .to_be_bytes()
                    .to_vec(),
            ]
            .concat()
        });
    let body = tables.iter().flat_map(|(_, data)| {
        let padding = data.len().next_multiple_of(4) - data.len();
        data.iter().copied().chain(std::iter::repeat_n(0, padding))
    });
    let mut out: Vec<u8> = header.into_iter().chain(directory).chain(body).collect();

    let head = tables
        .iter()
        .zip(&offsets)
        .find(|((tag, _), _)| tag == b"head")?
        .1;
    let at = usize::try_from(*head).ok()? + 8;
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
    out.get_mut(at..at + 4)?
        .copy_from_slice(&adjustment.to_be_bytes());
    Some(out)
}

/// The face `index` of a collection as a font file of its own, and the extension it gets (`otf`
/// for CFF outlines, else `ttf`).
fn split_face(collection: &[u8], index: u32) -> Option<(Vec<u8>, String)> {
    let start = usize::try_from(be32(collection, 12 + 4 * usize::try_from(index).ok()?)?).ok()?;
    let version: [u8; 4] = collection.get(start..start + 4)?.try_into().ok()?;
    let count = usize::from(be16(collection, start + 4)?);
    let tables = (0..count)
        .map(|i| {
            let record = start + 12 + 16 * i;
            let tag: [u8; 4] = collection.get(record..record + 4)?.try_into().ok()?;
            let offset = usize::try_from(be32(collection, record + 8)?).ok()?;
            let length = usize::try_from(be32(collection, record + 12)?).ok()?;
            let data = collection.get(offset..offset.checked_add(length)?)?;
            Some((tag, data))
        })
        .collect::<Option<Vec<_>>>()?;
    let extension = if &version == b"OTTO" { "otf" } else { "ttf" };
    Some((assemble(version, &tables)?, extension.to_owned()))
}

/// The bytes of Mac Roman from 0x80 on, as characters.
const MAC_ROMAN: [char; 128] = [
    'Ä', 'Å', 'Ç', 'É', 'Ñ', 'Ö', 'Ü', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ç', 'é', 'è', //
    'ê', 'ë', 'í', 'ì', 'î', 'ï', 'ñ', 'ó', 'ò', 'ô', 'ö', 'õ', 'ú', 'ù', 'û', 'ü', //
    '†', '°', '¢', '£', '§', '•', '¶', 'ß', '®', '©', '™', '´', '¨', '≠', 'Æ', 'Ø', //
    '∞', '±', '≤', '≥', '¥', 'µ', '∂', '∑', '∏', 'π', '∫', 'ª', 'º', 'Ω', 'æ', 'ø', //
    '¿', '¡', '¬', '√', 'ƒ', '≈', '∆', '«', '»', '…', '\u{a0}', 'À', 'Ã', 'Õ', 'Œ', 'œ', //
    '–', '—', '“', '”', '‘', '’', '÷', '◊', 'ÿ', 'Ÿ', '⁄', '€', '‹', '›', 'ﬁ', 'ﬂ', //
    '‡', '·', '‚', '„', '‰', 'Â', 'Ê', 'Á', 'Ë', 'È', 'Í', 'Î', 'Ï', 'Ì', 'Ó', 'Ô', //
    '\u{f8ff}', 'Ò', 'Ú', 'Û', 'Ù', 'ı', 'ˆ', '˜', '¯', '˘', '˙', '˚', '¸', '˝', '˛', 'ˇ',
];

/// The text of a name record: UTF-16 for Unicode and Windows, Mac Roman for the Macintosh
/// platform's Roman script (some fonts, the Futura of macOS, have a name only in that).
fn decode_name(name: &ttf_parser::name::Name) -> Option<String> {
    if name.is_unicode() {
        return name.to_string();
    }
    let mac_roman = name.platform_id == PlatformId::Macintosh && name.encoding_id == 0;
    mac_roman.then(|| {
        name.name
            .iter()
            .map(|&b| {
                if b < 0x80 {
                    char::from(b)
                } else {
                    MAC_ROMAN[usize::from(b - 0x80)]
                }
            })
            .collect()
    })
}

/// Whether a record is in English or in no language in particular (the Unicode platform has no
/// language field), as opposed to another language.
fn is_english_or_neutral(name: &ttf_parser::name::Name) -> bool {
    match name.platform_id {
        PlatformId::Windows => name.language_id & 0x3ff == 0x09,
        PlatformId::Macintosh => name.language_id == 0,
        _ => name.is_unicode(),
    }
}

/// A name of the font by the first of `ids` that has one: English (or language-less) records
/// first, in any of the ids, and only then other languages. The first record by sort order is
/// often not English (a Chinese style name on macOS fonts that have only that in UTF-16).
fn read_name(bytes: &[u8], ids: [u16; 2]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    [true, false].iter().find_map(|english| {
        ids.iter().find_map(|id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == *id && is_english_or_neutral(n) == *english)
                .find_map(|n| decode_name(&n).filter(|text| !text.is_empty()))
        })
    })
}

/// Words that only describe a style at the end of a family name.
const STYLE_SUFFIXES: [&str; 32] = [
    "normal",
    "italic",
    "oblique",
    "slanted",
    "thin",
    "th",
    "hairline",
    "light",
    "lt",
    "regular",
    "medium",
    "med",
    "md",
    "bold",
    "bd",
    "demi",
    "extb",
    "black",
    "blk",
    "bk",
    "heavy",
    "narrow",
    "condensed",
    "cond",
    "cn",
    "cd",
    "compressed",
    "expanded",
    "exp",
    "vf",
    "var",
    "variable",
];

/// Words that can stand in front of a style suffix (`Extra Bold`, `Semi Light`).
const STYLE_MODIFIERS: [&str; 10] = [
    "extra", "ext", "ex", "x", "semi", "sem", "sm", "demi", "dem", "ultra",
];

/// One more style word off the end of a lowercase family name, `None` when there is none to take.
fn trim_style(name: &str) -> Option<&str> {
    let bare = std::iter::successors(Some(name), |t| {
        STYLE_SUFFIXES.iter().find_map(|s| t.strip_suffix(s))
    })
    .last()
    .filter(|bare| bare.len() < name.len())?;
    let separated = bare.strip_suffix([' ', '-', '_']);
    // a modifier counts only when a separator sets it off, or `Box` would lose its `x`
    let modified = STYLE_MODIFIERS
        .iter()
        .find_map(|m| separated.unwrap_or(bare).strip_suffix(m))
        .and_then(|rest| rest.strip_suffix([' ', '-', '_']));
    modified.or(separated)
}

/// The family the way Typst sees it. Typst files a font under the first record of name id 1 in
/// table order, whatever its language, minus the trailing style words (`Open Sans Light` is
/// `Open Sans`); it ignores the typographic family (id 16). A family that is stored differently
/// is a name the layout cannot use: Typst does not find the font under it. Mirrors
/// `FontInfo::from_ttf` of typst-library 0.15 (bundled version in `scripts/sidecars.json`),
/// without its list of per-font exceptions (a few Noto and Archivo files).
fn family_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let family = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == name_id::FAMILY)
        .find_map(|n| decode_name(&n))?;
    let family = family.trim().trim_start_matches('.');
    let lower = family.to_ascii_lowercase();
    // lowercasing ASCII keeps every byte position, so the length cuts the original
    let len = std::iter::successors(Some(lower.as_str()), |t| trim_style(t))
        .last()
        .map_or(0, str::len);
    Some(family[..len].to_owned()).filter(|family| !family.is_empty())
}

/// The weight and style the font calls itself (`Regular`, `Bold Italic`), a variable font says what
/// its default is. Only a label: Typst takes weight and slant from the font's tables, so unlike the
/// family this one is read for a person, in English where there is any.
fn style_name(bytes: &[u8]) -> Option<String> {
    read_name(bytes, [name_id::TYPOGRAPHIC_SUBFAMILY, name_id::SUBFAMILY])
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
        let is = |extensions: &[&str]| {
            Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
        };
        (name.starts_with("background-") && is(&["svg"]))
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

/// Fonts made in memory for the tests: a free font with a name table of choice, and a collection
/// of several such fonts.
#[cfg(test)]
pub(crate) mod test_fonts {
    use super::*;

    /// A name record: platform, encoding, language, name id, bytes
    pub type Record = (u16, u16, u16, u16, Vec<u8>);

    pub fn open_sans() -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/OpenSans.ttf"))
            .unwrap()
    }

    pub fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    /// A name table (format 0) with the records in the order given
    fn name_table(records: &[Record]) -> Vec<u8> {
        let count = u16::try_from(records.len()).unwrap();
        let mut storage = Vec::new();
        let directory: Vec<u8> = records
            .iter()
            .flat_map(|(platform, encoding, language, id, text)| {
                let offset = u16::try_from(storage.len()).unwrap();
                storage.extend(text);
                [
                    *platform,
                    *encoding,
                    *language,
                    *id,
                    u16::try_from(text.len()).unwrap(),
                    offset,
                ]
                .into_iter()
                .flat_map(u16::to_be_bytes)
            })
            .collect();
        [0, count, 6 + 12 * count]
            .into_iter()
            .flat_map(u16::to_be_bytes)
            .chain(directory)
            .chain(storage)
            .collect()
    }

    /// The tables of the first face of a font file
    fn tables_of(font: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        (0..usize::from(be16(font, 4).unwrap()))
            .map(|i| {
                let record = 12 + 16 * i;
                let offset = usize::try_from(be32(font, record + 8).unwrap()).unwrap();
                let length = usize::try_from(be32(font, record + 12).unwrap()).unwrap();
                (
                    font[record..record + 4].try_into().unwrap(),
                    font[offset..offset + length].to_vec(),
                )
            })
            .collect()
    }

    /// The font with another name table
    pub fn with_names(font: &[u8], records: &[Record]) -> Vec<u8> {
        let name = name_table(records);
        let mut tables = tables_of(font);
        tables
            .iter_mut()
            .filter(|(tag, _)| tag == b"name")
            .for_each(|(_, data)| data.clone_from(&name));
        let borrowed: Vec<_> = tables
            .iter()
            .map(|(tag, data)| (*tag, data.as_slice()))
            .collect();
        assemble([0, 1, 0, 0], &borrowed).unwrap()
    }

    /// A collection of single fonts; a table several faces have is stored once, as the collections
    /// of the system do
    pub fn collection(fonts: &[Vec<u8>]) -> Vec<u8> {
        let faces: Vec<_> = fonts.iter().map(|f| tables_of(f)).collect();
        let header = 12 + 4 * faces.len();
        let data_start = header + faces.iter().map(|t| 12 + 16 * t.len()).sum::<usize>();
        let mut data: Vec<u8> = Vec::new();
        let mut placed: Vec<(&Vec<u8>, usize)> = Vec::new();
        let directories: Vec<Vec<u8>> = faces
            .iter()
            .map(|tables| {
                let mut directory = vec![0, 1, 0, 0];
                directory.extend(u16::try_from(tables.len()).unwrap().to_be_bytes());
                directory.extend([0u8; 6]);
                tables.iter().for_each(|(tag, table)| {
                    let known = placed
                        .iter()
                        .find(|(known, _)| *known == table)
                        .map(|(_, at)| *at);
                    let at = known.unwrap_or_else(|| {
                        let at = data_start + data.len();
                        data.extend(table);
                        data.resize(data.len().next_multiple_of(4), 0);
                        placed.push((table, at));
                        at
                    });
                    directory.extend(tag);
                    directory.extend(checksum(table).to_be_bytes());
                    directory.extend(u32::try_from(at).unwrap().to_be_bytes());
                    directory.extend(u32::try_from(table.len()).unwrap().to_be_bytes());
                });
                directory
            })
            .collect();
        let starts = directories.iter().scan(header, |at, directory| {
            let start = *at;
            *at += directory.len();
            Some(start)
        });
        let mut out = b"ttcf".to_vec();
        out.extend(0x0001_0000u32.to_be_bytes());
        out.extend(u32::try_from(faces.len()).unwrap().to_be_bytes());
        starts.for_each(|start| out.extend(u32::try_from(start).unwrap().to_be_bytes()));
        directories
            .iter()
            .for_each(|directory| out.extend(directory));
        out.extend(data);
        out
    }
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
        let font = import_font(&bundled("OpenSans.ttf"), dir.path())
            .unwrap()
            .remove(0);
        assert_eq!(font.family, "Open Sans");
        assert_eq!(font.style, "Regular");
        assert!(font.path.starts_with(dir.path()));
    }

    #[test]
    fn files_no_layout_points_at_are_pruned_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let kept = import_font(&bundled("OpenSans.ttf"), &dir.path().join("fonts"))
            .unwrap()
            .remove(0);
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

    use super::test_fonts::{collection, open_sans, utf16, with_names, Record};

    /// What the import makes of a font that has these name records
    fn named(records: &[Record]) -> (Option<String>, Option<String>) {
        let font = with_names(&open_sans(), records);
        (family_name(&font), style_name(&font))
    }

    fn windows(language: u16, id: u16, text: &str) -> Record {
        (3, 1, language, id, utf16(text))
    }

    #[test]
    fn the_family_is_the_one_typst_files_the_font_under() {
        let family = |records: &[Record]| named(records).0;

        // the first record of the family wins, whatever its language: that is Typst's pick, and a
        // name it does not know cannot be used in a layout
        assert_eq!(
            family(&[
                windows(0x0407, 1, "Schrift"),
                windows(0x0409, 1, "Font"),
                windows(0x0407, 2, "Standard"),
                windows(0x0409, 2, "Regular"),
            ])
            .as_deref(),
            Some("Schrift")
        );
        assert_eq!(
            family(&[windows(0x0804, 1, "富图拉"), windows(0x0409, 1, "Futura")]).as_deref(),
            Some("富图拉")
        );
        // the Macintosh platform sorts before Windows
        assert_eq!(
            family(&[
                (1, 0, 0, 1, b"Mac Name".to_vec()),
                windows(0x0409, 1, "Windows Name"),
            ])
            .as_deref(),
            Some("Mac Name")
        );
        // the typographic family is not used, it would group more than weights and widths
        assert_eq!(
            family(&[
                windows(0x0409, 1, "Noto Sans Display"),
                windows(0x0409, 16, "Noto Sans"),
            ])
            .as_deref(),
            Some("Noto Sans Display")
        );
        // words that only name a style come off the end, as Typst does it
        let trimmed = |name: &str| family(&[windows(0x0409, 1, name)]);
        assert_eq!(trimmed("Open Sans Light").as_deref(), Some("Open Sans"));
        assert_eq!(
            trimmed("Open Sans Extra Bold").as_deref(),
            Some("Open Sans")
        );
        assert_eq!(trimmed("Futura-Medium").as_deref(), Some("Futura"));
        assert_eq!(trimmed(".SF NS Italic").as_deref(), Some("SF NS"));
        // a modifier alone is part of the name, and so is a name made of nothing but a style word
        assert_eq!(trimmed("Box").as_deref(), Some("Box"));
        assert_eq!(trimmed("Hex Bold").as_deref(), Some("Hex"));
        assert_eq!(trimmed("Regular").as_deref(), Some("Regular"));
        assert_eq!(family(&[]), None);
    }

    #[test]
    fn the_style_prefers_english_and_the_typographic_name() {
        let style = |records: &[Record]| named(records).1;

        // a Chinese record that sorts first is not the name of the style
        assert_eq!(
            style(&[
                windows(0x0804, 1, "富图拉"),
                windows(0x0804, 2, "中黑"),
                windows(0x0409, 1, "Futura"),
                windows(0x0409, 2, "Medium"),
            ])
            .as_deref(),
            Some("Medium")
        );
        // other kinds of English count, and the typographic names come before the plain ones
        assert_eq!(
            style(&[
                windows(0x0809, 1, "Open Sans Light"),
                windows(0x0809, 2, "Regular"),
                windows(0x0809, 16, "Open Sans"),
                windows(0x0809, 17, "Light"),
            ])
            .as_deref(),
            Some("Light")
        );
        // the plain name in English beats a typographic one in another language
        assert_eq!(
            style(&[
                windows(0x0804, 16, "富图拉"),
                windows(0x0804, 17, "中黑"),
                windows(0x0409, 1, "Futura"),
                windows(0x0409, 2, "Medium"),
            ])
            .as_deref(),
            Some("Medium")
        );
        // a Macintosh record in another language is no English one
        assert_eq!(
            style(&[
                (1, 0, 1, 1, b"Futura fr".to_vec()),
                (1, 0, 1, 2, b"Moyen".to_vec()),
                windows(0x0409, 1, "Futura"),
                windows(0x0409, 2, "Medium"),
            ])
            .as_deref(),
            Some("Medium")
        );
        // with nothing in English the name there is is taken
        assert_eq!(
            style(&[windows(0x0804, 1, "富图拉"), windows(0x0804, 2, "中黑")]).as_deref(),
            Some("中黑")
        );
        assert_eq!(style(&[]), None);
    }

    #[test]
    fn a_font_that_has_its_names_only_in_mac_roman_is_a_font() {
        // 0x8e is the é of Mac Roman, the records are English and no UTF-16 follows
        let records = [
            (1, 0, 0, 1, b"Caf\x8e Sans".to_vec()),
            (1, 0, 0, 2, b"Medium".to_vec()),
        ];
        assert_eq!(
            named(&records),
            (Some("Café Sans".into()), Some("Medium".into()))
        );

        let dir = tempfile::tempdir().unwrap();
        let font = with_names(&open_sans(), &records);
        let fonts = store_font("cafe.ttf", &font, dir.path()).unwrap();
        assert_eq!(
            fonts.iter().map(|f| f.family.as_str()).collect::<Vec<_>>(),
            ["Café Sans"]
        );
    }

    #[test]
    fn a_single_font_is_kept_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = open_sans();

        let fonts = store_font("OpenSans.ttf", &bytes, dir.path()).unwrap();

        // the same name as before collections were split, so stored layouts keep their files
        assert_eq!(
            fonts[0].path,
            dir.path().join(format!("{}.ttf", fingerprint(&bytes)))
        );
        assert_eq!(std::fs::read(&fonts[0].path).unwrap(), bytes);
    }

    #[test]
    fn every_face_of_a_collection_becomes_a_font_of_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let regular = with_names(
            &open_sans(),
            &[
                windows(0x0409, 1, "Test Sans"),
                windows(0x0409, 2, "Medium"),
            ],
        );
        let bold = with_names(
            &open_sans(),
            &[windows(0x0409, 1, "Test Sans"), windows(0x0409, 2, "Bold")],
        );
        let condensed = with_names(
            &open_sans(),
            &[
                (1, 0, 0, 1, b"Test Sans".to_vec()),
                (1, 0, 0, 2, b"Condensed".to_vec()),
            ],
        );
        let faces = collection(&[regular, bold, condensed]);

        let fonts = store_font("TestSans.ttc", &faces, dir.path()).unwrap();

        assert_eq!(
            fonts
                .iter()
                .map(|f| (f.family.as_str(), f.style.as_str()))
                .collect::<Vec<_>>(),
            [
                ("Test Sans", "Medium"),
                ("Test Sans", "Bold"),
                ("Test Sans", "Condensed")
            ]
        );
        let source = open_sans();
        let original = ttf_parser::Face::parse(&source, 0).unwrap();
        let paths: HashSet<_> = fonts.iter().map(|f| &f.path).collect();
        assert_eq!(paths.len(), 3);
        for font in &fonts {
            let bytes = std::fs::read(&font.path).unwrap();
            // a font file, no collection: the checksum of the whole file comes out
            assert_eq!(font.path.extension().and_then(|e| e.to_str()), Some("ttf"));
            assert_eq!(ttf_parser::fonts_in_collection(&bytes), None);
            assert_eq!(checksum(&bytes), 0xB1B0_AFBA);
            let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
            assert_eq!(face.number_of_glyphs(), original.number_of_glyphs());
            assert_eq!(face.units_per_em(), original.units_per_em());
        }
    }

    #[test]
    fn a_damaged_or_overlong_collection_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let faces = collection(&[open_sans(), open_sans()]);

        // the first face points beyond the end of the file
        let mut damaged = faces.clone();
        damaged[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(store_font("a.ttc", &damaged, dir.path()).is_err());
        assert!(store_font("a.ttc", &faces[..faces.len() / 2], dir.path()).is_err());

        // a count that asks for more than anybody has
        let mut many = faces;
        many[8..12].copy_from_slice(&1000u32.to_be_bytes());
        assert!(store_font("a.ttc", &many, dir.path()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    /// The Futura of macOS: five faces in a collection, the family only as a Mac Roman record and
    /// styles in several languages. Read only, never kept in the repository.
    /// `FONTS_OUT=/some/dir` keeps the faces to look at.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "needs the Futura of macOS"]
    fn the_futura_collection_of_macos_comes_in_with_all_its_faces() {
        let source = Path::new("/System/Library/Fonts/Supplemental/Futura.ttc");
        let kept = tempfile::tempdir().unwrap();
        let dir = std::env::var("FONTS_OUT").map_or_else(|_| kept.path().to_owned(), Into::into);

        let fonts = import_font(source, &dir).unwrap();

        assert_eq!(fonts.len(), 5, "{fonts:#?}");
        assert!(fonts.iter().all(|f| f.family == "Futura"), "{fonts:#?}");
        let styles: Vec<_> = fonts.iter().map(|f| f.style.as_str()).collect();
        assert_eq!(
            styles,
            [
                "Medium",
                "Medium Italic",
                "Bold",
                "Condensed Medium",
                "Condensed ExtraBold"
            ]
        );
    }
}
