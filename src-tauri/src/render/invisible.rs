//! Marks for what cannot be seen, for the sample of a layout with its guides on: line breaks,
//! spaces, the columns of a tab and the characters that come along with a copy and paste (no-break
//! and thin spaces, zero-width characters, soft hyphens, bidi controls, control characters). The
//! text is cut at each of them into runs that carry a `Mark`; the template draws the mark next to
//! the character. Without the guides nothing here runs, so documents never see a mark.

use serde::Serialize;

use crate::invisible::label;

use super::parts::{Line, PartView, PartViewContent, Run};
use super::view::PageView;

/// What the template draws for a run
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Mark {
    /// A dot on a normal space, which keeps its width and its place to break
    Space,
    /// An arrow where a tab ended a column
    Column,
    /// A pilcrow at the end of a line
    LineEnd,
    /// A coloured label next to a character; the character stays, so shaping, line breaking and
    /// the direction of the text are as they are in the document
    Badge { label: &'static str },
}

fn mark_of(c: char) -> Option<Mark> {
    if c == ' ' {
        Some(Mark::Space)
    } else {
        label(c).map(|label| Mark::Badge { label })
    }
}

/// A run cut at every character that gets a mark: the plain text between stays together, each
/// marked character is a run of its own.
fn reveal_run(run: Run) -> Vec<Run> {
    let own = |text: String, mark| Run {
        text,
        bold: run.bold,
        italic: run.italic,
        mark,
    };
    let mut runs = Vec::new();
    let mut plain = String::new();
    for c in run.text.chars() {
        match mark_of(c) {
            Some(mark) => {
                if !plain.is_empty() {
                    runs.push(own(std::mem::take(&mut plain), None));
                }
                runs.push(own(c.to_string(), Some(mark)));
            }
            None => plain.push(c),
        }
    }
    if !plain.is_empty() {
        runs.push(own(plain, None));
    }
    runs
}

fn mark_run(mark: Mark) -> Run {
    Run {
        text: String::new(),
        bold: false,
        italic: false,
        mark: Some(mark),
    }
}

/// Lines with their characters marked, an arrow after each column but the last and a pilcrow at
/// the end of the last one.
pub fn reveal_lines(lines: Vec<Line>) -> Vec<Line> {
    lines
        .into_iter()
        .map(|line| {
            let last = line.len().saturating_sub(1);
            line.into_iter()
                .enumerate()
                .map(|(i, column)| {
                    let mut runs: Vec<Run> = column.into_iter().flat_map(reveal_run).collect();
                    runs.push(mark_run(if i == last {
                        Mark::LineEnd
                    } else {
                        Mark::Column
                    }));
                    runs
                })
                .collect()
        })
        .collect()
}

/// The text of a page number marked the same way. A tab in it is no column, so it gets a badge like
/// any character that is not a space.
fn reveal_text(text: &str) -> Vec<Run> {
    let mut runs = reveal_run(Run {
        text: text.to_owned(),
        bold: false,
        italic: false,
        mark: None,
    });
    runs.push(mark_run(Mark::LineEnd));
    runs
}

fn reveal_parts(parts: &mut [PartView]) {
    parts.iter_mut().for_each(|part| match &mut part.content {
        PartViewContent::Text { lines, .. } => *lines = reveal_lines(std::mem::take(lines)),
        PartViewContent::PageNumber { text, marked, .. } => *marked = Some(reveal_text(text)),
        _ => {}
    });
}

/// Marks the texts a layout prints from its own: text parts and page numbers, on every page setup.
/// The data of the invoice that is not in a part (title, positions, totals) stays as it is.
pub fn reveal_page(page: &mut PageView) {
    reveal_parts(&mut page.parts);
    if let Some(parts) = page.first_parts.as_mut() {
        reveal_parts(parts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    use crate::layout::{Align, Anchor};

    fn run(text: &str) -> Run {
        Run {
            text: text.into(),
            bold: false,
            italic: false,
            mark: None,
        }
    }

    #[test]
    fn every_character_class_gets_its_own_mark() {
        let badges = [
            ('\u{00A0}', "NBSP"),
            ('\u{202F}', "NNBSP"),
            ('\u{2000}', "NQSP"),
            ('\u{2001}', "MQSP"),
            ('\u{2002}', "ENSP"),
            ('\u{2003}', "EMSP"),
            ('\u{2004}', "3/MSP"),
            ('\u{2005}', "4/MSP"),
            ('\u{2006}', "6/MSP"),
            ('\u{2007}', "FSP"),
            ('\u{2008}', "PSP"),
            ('\u{2009}', "THSP"),
            ('\u{200A}', "HSP"),
            ('\u{205F}', "MMSP"),
            ('\u{3000}', "IDSP"),
            ('\u{200B}', "ZWSP"),
            ('\u{200C}', "ZWNJ"),
            ('\u{200D}', "ZWJ"),
            ('\u{2060}', "WJ"),
            ('\u{FEFF}', "BOM"),
            ('\u{00AD}', "SHY"),
            ('\u{2028}', "LS"),
            ('\u{2029}', "PS"),
            ('\u{200E}', "LRM"),
            ('\u{200F}', "RLM"),
            ('\u{061C}', "ALM"),
            ('\u{202A}', "LRE"),
            ('\u{202B}', "RLE"),
            ('\u{202C}', "PDF"),
            ('\u{202D}', "LRO"),
            ('\u{202E}', "RLO"),
            ('\u{2066}', "LRI"),
            ('\u{2067}', "RLI"),
            ('\u{2068}', "FSI"),
            ('\u{2069}', "PDI"),
            ('\u{180E}', "MVS"),
            ('\u{034F}', "CGJ"),
            ('\u{FFFC}', "OBJ"),
            ('\u{2011}', "NBHY"),
            ('\u{0}', "NUL"),
            ('\u{1}', "SOH"),
            ('\u{9}', "TAB"),
            ('\u{B}', "VT"),
            ('\u{D}', "CR"),
            ('\u{1F}', "US"),
            ('\u{7F}', "DEL"),
            ('\u{85}', "NEL"),
            ('\u{90}', "C1"),
        ];

        badges.iter().for_each(|(c, label)| {
            assert_eq!(
                mark_of(*c),
                Some(Mark::Badge { label }),
                "U+{:04X}",
                u32::from(*c)
            );
        });
        // no two of them look alike
        let labels: HashSet<_> = badges.iter().map(|(_, label)| *label).collect();
        assert_eq!(labels.len(), badges.len());
        assert_eq!(mark_of(' '), Some(Mark::Space));
    }

    #[test]
    fn what_can_be_seen_has_no_mark() {
        "Aa9 ß€ä–-./\\*{}:,;\u{2010}\u{2012}\u{2013}\u{2014}\u{2212}"
            .chars()
            .filter(|c| *c != ' ')
            .for_each(|c| assert_eq!(mark_of(c), None, "{c:?}"));
        assert_eq!(mark_of('\n'), None);
    }

    #[test]
    fn a_run_is_cut_at_the_marked_characters_and_keeps_its_style() {
        let runs = reveal_run(Run {
            bold: true,
            ..run("ab\u{00A0}c d")
        });

        let shown: Vec<_> = runs.iter().map(|r| (r.text.as_str(), r.mark)).collect();
        assert_eq!(
            shown,
            [
                ("ab", None),
                ("\u{00A0}", Some(Mark::Badge { label: "NBSP" })),
                ("c", None),
                (" ", Some(Mark::Space)),
                ("d", None),
            ]
        );
        assert!(runs.iter().all(|r| r.bold && !r.italic));
        // the characters are all still there
        let text: String = runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(text, "ab\u{00A0}c d");
    }

    #[test]
    fn lines_end_with_a_pilcrow_and_columns_with_an_arrow() {
        let revealed = reveal_lines(vec![
            vec![vec![run("Tel.")], vec![run("0221 1")]],
            vec![vec![]],
        ]);

        let marks = |line: &Line| -> Vec<Vec<Option<Mark>>> {
            line.iter()
                .map(|column| column.iter().map(|r| r.mark).collect())
                .collect()
        };
        assert_eq!(
            marks(&revealed[0]),
            [
                vec![None, Some(Mark::Column)],
                vec![None, Some(Mark::Space), None, Some(Mark::LineEnd)],
            ]
        );
        // an empty line is a pilcrow alone
        assert_eq!(marks(&revealed[1]), [vec![Some(Mark::LineEnd)]]);
        // the number of columns stays, the template sizes its grid by it
        assert_eq!(revealed[0].len(), 2);
    }

    fn text_part(text: &str) -> PartView {
        PartView {
            index: 0,
            anchor: Anchor::TopLeft,
            x: 0.0,
            y: 0.0,
            width: None,
            content: PartViewContent::Text {
                lines: super::super::parts::runs(text),
                family: None,
                size: 9.0,
                align: Align::Left,
                color: None,
            },
        }
    }

    fn number_part(text: &str) -> PartView {
        PartView {
            content: PartViewContent::PageNumber {
                text: text.into(),
                size: 9.0,
                align: Align::Left,
                several_pages_only: false,
                color: None,
                marked: None,
            },
            ..text_part("")
        }
    }

    #[test]
    fn texts_and_page_numbers_of_both_setups_are_marked() {
        let mut page = PageView {
            parts: vec![
                text_part("a b"),
                number_part("Seite {page}\u{00A0}von {pages}"),
            ],
            first_parts: Some(vec![text_part("c")]),
            ..PageView::default()
        };

        reveal_page(&mut page);

        let lines = |part: &PartView| match &part.content {
            PartViewContent::Text { lines, .. } => lines.clone(),
            _ => panic!("a text"),
        };
        assert_eq!(lines(&page.parts[0])[0][0].len(), 4);
        assert_eq!(lines(&page.first_parts.as_ref().unwrap()[0])[0][0].len(), 2);
        let PartViewContent::PageNumber { marked, text, .. } = &page.parts[1].content else {
            panic!("a page number")
        };
        // the placeholders stay whole for the template to fill
        assert_eq!(text, "Seite {page}\u{00A0}von {pages}");
        let marked = marked.as_ref().unwrap();
        assert!(marked
            .iter()
            .any(|r| r.text == "{page}" || r.text.ends_with("{page}")));
        assert!(marked
            .iter()
            .any(|r| r.mark == Some(Mark::Badge { label: "NBSP" })));
        assert_eq!(marked.last().unwrap().mark, Some(Mark::LineEnd));
    }

    #[test]
    fn the_marks_of_a_run_are_only_serialized_when_there_are_any() {
        assert!(!serde_json::to_string(&run("x")).unwrap().contains("mark"));
        let marked = Run {
            mark: Some(Mark::Badge { label: "ZWSP" }),
            ..run("\u{200B}")
        };
        assert!(serde_json::to_string(&marked)
            .unwrap()
            .contains(r#""mark":{"kind":"badge","label":"ZWSP"}"#));
    }
}
