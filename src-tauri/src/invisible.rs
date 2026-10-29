//! The characters a copy and paste brings along that cannot be seen or behave unlike they look:
//! their short codes for the marks of the layout preview, and the cleaning of what is typed in.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// Names of the control characters U+0000 to U+001F
const CONTROLS: [&str; 32] = [
    "NUL", "SOH", "STX", "ETX", "EOT", "ENQ", "ACK", "BEL", "BS", "TAB", "LF", "VT", "FF", "CR",
    "SO", "SI", "DLE", "DC1", "DC2", "DC3", "DC4", "NAK", "SYN", "ETB", "CAN", "EM", "SUB", "ESC",
    "FS", "GS", "RS", "US",
];

#[derive(Deserialize)]
struct Entry {
    code: String,
    label: String,
    /// What cleaning makes of it
    to: String,
}

#[derive(Deserialize)]
struct TableFile {
    chars: Vec<Entry>,
}

/// Shared with the text field of the frontend, which cleans what the preview marks the same way
const TABLE_JSON: &str = include_str!("../data/invisible-chars.json");

fn table() -> &'static HashMap<char, Entry> {
    static TABLE: OnceLock<HashMap<char, Entry>> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str::<TableFile>(TABLE_JSON)
            .expect("data/invisible-chars.json is valid, a test reads it")
            .chars
            .into_iter()
            .map(|entry| {
                let c = u32::from_str_radix(&entry.code, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .expect("data/invisible-chars.json has code points, a test reads it");
                (c, entry)
            })
            .collect()
    })
}

/// The short code of a character that is invisible or behaves unlike it looks, the abbreviations
/// the Unicode names list where there are some. A normal space is no badge but a dot.
pub fn label(c: char) -> Option<&'static str> {
    if let Some(entry) = table().get(&c) {
        return Some(&entry.label);
    }
    Some(match c {
        // the lines are cut before, a line feed is never in a run
        '\n' => return None,
        '\u{7F}' => "DEL",
        '\u{85}' => "NEL",
        '\u{80}'..='\u{9F}' => "C1",
        c if (c as u32) < 0x20 => CONTROLS[c as usize],
        _ => return None,
    })
}

fn is_control(c: char) -> bool {
    c.is_control() && c != '\t' && c != '\n'
}

/// The text with special spaces as normal ones, line separators as line breaks and what has no
/// width, bidi and control characters gone, like the button under a text field. A single line also
/// turns line breaks and tabs into spaces, a field of one line has no use for them.
pub fn clean(text: &str, single_line: bool) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| match table().get(&c) {
            Some(entry) => entry.to.clone(),
            None if is_control(c) => String::new(),
            None => c.to_string(),
        })
        .collect();
    if single_line {
        cleaned.replace(['\n', '\t'], " ")
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    #[test]
    fn the_shared_table_has_distinct_labels_apart_from_the_controls() {
        let file: TableFile = serde_json::from_str(TABLE_JSON).unwrap();
        assert_eq!(
            table().len(),
            file.chars.len(),
            "a code point is listed twice"
        );

        let labels: HashSet<_> = table().values().map(|e| e.label.as_str()).collect();
        assert_eq!(labels.len(), table().len(), "a label is used twice");
        assert!(table().keys().all(|c| *c != ' ' && !c.is_control()));
        assert!(CONTROLS
            .iter()
            .chain(&["DEL", "NEL", "C1"])
            .all(|name| !labels.contains(name)));
    }

    #[test]
    fn cleaning_matches_the_button_of_the_text_field() {
        assert_eq!(clean("+49\u{00A0}221\u{202F}1", false), "+49 221 1");
        assert_eq!(
            clean("E\u{2011}Mail\u{200B}\u{FFFC}\u{0007}", false),
            "E-Mail"
        );
        assert_eq!(clean("HRB 1\u{2028}GF: A", false), "HRB 1\nGF: A");
        let plain = "Mustermann GmbH\nGF: A\tB – ß";
        assert_eq!(clean(plain, false), plain);
    }

    #[test]
    fn a_single_line_keeps_no_line_breaks_or_tabs() {
        assert_eq!(clean("Musterstr.\u{2028}1\n\t2\r", true), "Musterstr. 1  2");
    }
}
