//! The parts of a layout as the template wants them: the data of the invoice already put into
//! them, text already cut into runs, pictures already copied next to the template. The template
//! only places them.

use anyhow::Result;
use serde::Serialize;

use super::view::{InvoiceView, PageView};
use crate::layout::{
    Align, Anchor, CustomLayout, ImageFile, InfoField, PageSetup, Part, PartContent,
};

/// A raster image below this many dots per inch at its placed size looks blurry in print.
pub const MIN_DPI: f64 = 150.0;

/// A piece of text that is set the same way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Serialize)]
pub struct InfoRow {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PartViewContent {
    Infos {
        rows: Vec<InfoRow>,
        labels: bool,
        align: Align,
        size: f64,
        color: Option<String>,
    },
    Address {
        sender: Option<String>,
        lines: Vec<String>,
        size: f64,
        color: Option<String>,
    },
    Image {
        /// File name next to the template
        file: String,
        /// Too few pixels for its size, the preview marks it
        low_res: bool,
    },
    Text {
        lines: Vec<Vec<Run>>,
        family: Option<String>,
        size: f64,
        align: Align,
        color: Option<String>,
    },
    PageNumber {
        text: String,
        size: f64,
        align: Align,
        color: Option<String>,
    },
    Line {
        thickness: f64,
        color: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct PartView {
    /// Where it is in the list of the setup, whatever order it is drawn in
    pub index: usize,
    pub anchor: Anchor,
    pub x: f64,
    pub y: f64,
    pub width: Option<f64>,
    #[serde(flatten)]
    pub content: PartViewContent,
}

/// Text with `**bold**` and `*italic*` as lines of runs. A backslash keeps a star as it is.
pub fn runs(text: &str) -> Vec<Vec<Run>> {
    text.lines()
        .map(|line| {
            let mut out: Vec<Run> = Vec::new();
            let (mut bold, mut italic) = (false, false);
            let mut current = String::new();
            let flush = |current: &mut String, out: &mut Vec<Run>, bold: bool, italic: bool| {
                if !current.is_empty() {
                    out.push(Run {
                        text: std::mem::take(current),
                        bold,
                        italic,
                    });
                }
            };
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '\\' if chars.peek() == Some(&'*') => {
                        current.push('*');
                        chars.next();
                    }
                    '*' => {
                        flush(&mut current, &mut out, bold, italic);
                        if chars.peek() == Some(&'*') {
                            chars.next();
                            bold = !bold;
                        } else {
                            italic = !italic;
                        }
                    }
                    other => current.push(other),
                }
            }
            flush(&mut current, &mut out, bold, italic);
            out
        })
        .collect()
}

/// Puts the data of the company into the `{placeholders}` of runs. A value of several lines
/// breaks the line it stands in.
pub fn fill(lines: Vec<Vec<Run>>, value: &dyn Fn(&str) -> Option<String>) -> Vec<Vec<Run>> {
    lines
        .into_iter()
        .flat_map(|line| {
            let mut result: Vec<Vec<Run>> = vec![Vec::new()];
            for run in line {
                let mut rest = run.text.as_str();
                while !rest.is_empty() {
                    let found = rest.find('{').and_then(|open| {
                        let close = rest[open..].find('}')? + open;
                        Some((open, close, value(&rest[open + 1..close])?))
                    });
                    let (before, replacement, after) = match found {
                        Some((open, close, text)) => {
                            (&rest[..open], Some(text), &rest[close + 1..])
                        }
                        None => (rest, None, ""),
                    };
                    let push = |text: &str, result: &mut Vec<Vec<Run>>| {
                        if !text.is_empty() {
                            result.last_mut().expect("a line").push(Run {
                                text: text.to_owned(),
                                bold: run.bold,
                                italic: run.italic,
                            });
                        }
                    };
                    push(before, &mut result);
                    if let Some(text) = replacement {
                        for (i, part) in text.split('\n').enumerate() {
                            if i > 0 {
                                result.push(Vec::new());
                            }
                            push(part, &mut result);
                        }
                    }
                    rest = after;
                }
            }
            result
        })
        .collect()
}

fn info(view: &InvoiceView, field: InfoField, long: bool) -> Option<InfoRow> {
    let key = match field {
        InfoField::Number => "number",
        InfoField::Date => "date",
        InfoField::ValidUntil => "valid-until",
        InfoField::ServiceDate => "service-date",
        InfoField::DueDate => "due-date",
        InfoField::BuyerReference => "buyer-reference",
        InfoField::PrecedingInvoice => "preceding-invoice",
        InfoField::OfferReference => "offer-reference",
        InfoField::TaxNumber => "tax-number",
        InfoField::VatId => "vat-id",
    };
    view.infos.iter().find(|i| i.key == key).map(|i| InfoRow {
        label: i.label.clone(),
        value: if long {
            i.value_long.clone()
        } else {
            i.value.clone()
        },
    })
}

/// The dots per inch of an image of `pixels` across that is `millimetres` wide on the page.
fn dpi(pixels: u32, millimetres: f64) -> f64 {
    f64::from(pixels) / (millimetres / 25.4)
}

fn part_view(
    index: usize,
    part: &Part,
    view: &InvoiceView,
    images: &mut dyn FnMut(&ImageFile) -> Result<String>,
) -> Result<Option<PartView>> {
    let content = match &part.content {
        PartContent::Infos {
            fields,
            labels,
            align,
            long_dates,
            size,
            color,
        } => PartViewContent::Infos {
            rows: fields
                .iter()
                .filter_map(|f| info(view, *f, *long_dates))
                .collect(),
            labels: *labels,
            align: *align,
            size: *size,
            color: color.clone(),
        },
        PartContent::Address {
            sender_line,
            size,
            color,
        } => PartViewContent::Address {
            sender: sender_line
                .then(|| format!("{} · {}", view.seller.name, view.seller.address.join(" · "))),
            lines: std::iter::once(view.buyer.name.clone())
                .chain(view.buyer.address.iter().cloned())
                .collect(),
            size: *size,
            color: color.clone(),
        },
        PartContent::Image { image } => {
            let Some(image) = image else { return Ok(None) };
            let width = part.width.unwrap_or(40.0);
            return Ok(Some(PartView {
                index,
                anchor: part.anchor,
                x: part.x,
                y: part.y,
                width: Some(width),
                content: PartViewContent::Image {
                    file: images(image)?,
                    low_res: !image.vector && dpi(image.width_px, width) < MIN_DPI,
                },
            }));
        }
        PartContent::Text {
            text,
            family,
            size,
            align,
            color,
        } => PartViewContent::Text {
            lines: fill(runs(text), &|name| placeholder(view, name)),
            family: family.clone(),
            size: *size,
            align: *align,
            color: color.clone(),
        },
        PartContent::PageNumber {
            text,
            size,
            align,
            color,
        } => PartViewContent::PageNumber {
            text: text.clone(),
            size: *size,
            align: *align,
            color: color.clone(),
        },
        PartContent::Line { thickness, color } => PartViewContent::Line {
            thickness: *thickness,
            color: color.clone(),
        },
    };
    Ok(Some(PartView {
        index,
        anchor: part.anchor,
        x: part.x,
        y: part.y,
        width: part.width,
        content,
    }))
}

/// What a `{placeholder}` of a text part stands for.
fn placeholder(view: &InvoiceView, name: &str) -> Option<String> {
    match name {
        "seller_name" => Some(view.seller.name.clone()),
        "seller_address" => Some(view.seller.address.join("\n")),
        "seller_contact" => Some(view.seller.contact.join("\n")),
        "seller_registration" => Some(view.seller.registration.join("\n")),
        // layouts of older versions say `{footer}` for it
        "legal" | "footer" => Some(view.legal.clone().unwrap_or_default()),
        // the date an offer is valid until, nothing for a document without one
        "valid_until" => Some(
            view.infos
                .iter()
                .find(|i| i.key == "valid-until")
                .map(|i| i.value_long.clone())
                .unwrap_or_default(),
        ),
        "bank" => Some(
            view.bank
                .iter()
                .map(|b| format!("{}: {}", b.label, b.value))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}

fn setup_parts(
    setup: &PageSetup,
    view: &InvoiceView,
    images: &mut dyn FnMut(&ImageFile) -> Result<String>,
) -> Result<Vec<PartView>> {
    let mut views = setup
        .parts
        .iter()
        .enumerate()
        .filter_map(|(index, part)| part_view(index, part, view, images).transpose())
        .collect::<Result<Vec<_>>>()?;
    // pictures lie behind the rest, a full page picture is the letterhead
    views.sort_by_key(|part| !matches!(part.content, PartViewContent::Image { .. }));
    Ok(views)
}

/// Margins, parts and font of a layout for the template. `images` copies a picture next to the
/// template and says how it is called.
pub fn page_view(
    layout: &CustomLayout,
    view: &InvoiceView,
    images: &mut dyn FnMut(&ImageFile) -> Result<String>,
) -> Result<PageView> {
    let first = layout.first.as_ref();
    Ok(PageView {
        font: layout.family().map(str::to_owned),
        margin: layout.page.margins,
        first_margin: first
            .map(|f| f.margins)
            .filter(|m| *m != layout.page.margins),
        parts: setup_parts(&layout.page, view, images)?,
        first_parts: first.map(|f| setup_parts(f, view, images)).transpose()?,
        guides: false,
        marks: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &[Vec<Run>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.iter().map(|r| r.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn bold_and_italic_become_runs_per_line() {
        let lines = runs("**Muster** GmbH\nein *Satz*");

        assert_eq!(texts(&lines), ["Muster GmbH", "ein Satz"]);
        assert!(lines[0][0].bold && !lines[0][1].bold);
        assert!(lines[1][1].italic && !lines[1][0].italic);
    }

    #[test]
    fn the_legal_information_is_legal_and_footer_for_layouts_of_older_versions() {
        use crate::draft::{build_invoice, fixtures};
        use crate::render::view::{build, Branding};

        let mut settings = fixtures::settings();
        settings.seller.legal_info = Some("HRB 1".into());
        let draft = fixtures::draft();
        let invoice = build_invoice(&draft, &settings, "RE-1").unwrap();
        let view = build(
            &invoice,
            &Branding::for_document(crate::model::Kind::Invoice, &draft, &settings, &[]),
            &crate::model::Format::default(),
        );

        assert_eq!(placeholder(&view, "legal").as_deref(), Some("HRB 1"));
        assert_eq!(placeholder(&view, "footer").as_deref(), Some("HRB 1"));
    }

    #[test]
    fn the_validity_of_an_offer_is_a_placeholder_and_empty_for_a_document_without() {
        use crate::draft::{build_invoice, fixtures};
        use crate::model::Kind;
        use crate::render::view::{build, Branding};

        let settings = fixtures::settings();
        let view_of = |kind: Kind| {
            let draft = settings.blank_draft(kind, "2026-10-17");
            let invoice = build_invoice(
                &crate::model::InvoiceDraft {
                    positions: fixtures::draft().positions,
                    customer: fixtures::customer(),
                    ..draft.clone()
                },
                &settings,
                "N-1",
            )
            .unwrap();
            build(
                &invoice,
                &Branding::for_document(kind, &draft, &settings, &[]),
                &crate::model::Format::default(),
            )
        };

        let offer = view_of(Kind::Offer);
        assert_eq!(
            placeholder(&offer, "valid_until").as_deref(),
            Some("16. November 2026")
        );
        assert_eq!(
            info(&offer, InfoField::ValidUntil, false).map(|row| row.value),
            Some("16.11.2026".into())
        );
        assert_eq!(
            placeholder(&view_of(Kind::Invoice), "valid_until").as_deref(),
            Some("")
        );
        assert!(info(&view_of(Kind::Invoice), InfoField::ValidUntil, false).is_none());
    }

    #[test]
    fn the_builtin_layout_shows_the_offers_an_invoice_names() {
        use crate::draft::{build_invoice, fixtures};
        use crate::model::InvoiceDraft;
        use crate::render::view::{build, Branding};

        let settings = fixtures::settings();
        let draft = InvoiceDraft {
            offer_reference: true,
            frozen_offers: vec!["AN-2026-003".into()],
            ..fixtures::draft()
        };
        let invoice = build_invoice(&draft, &settings, "RE-1").unwrap();
        let view = build(
            &invoice,
            &Branding::for_document(crate::model::Kind::Invoice, &draft, &settings, &[]),
            &crate::model::Format::default(),
        );
        let builtin = crate::layout::builtin();
        let shown: Vec<_> = builtin
            .page
            .parts
            .iter()
            .chain(builtin.first.iter().flat_map(|first| &first.parts))
            .filter_map(|part| match &part.content {
                PartContent::Infos { fields, .. } => Some(fields),
                _ => None,
            })
            .flatten()
            .filter_map(|field| info(&view, *field, false))
            .collect();

        let row = shown.iter().find(|row| row.label == "Zu Angebot").unwrap();
        assert_eq!(row.value, "AN-2026-003");
    }

    #[test]
    fn a_backslash_keeps_a_star() {
        assert_eq!(texts(&runs(r"5 \* 3")), ["5 * 3"]);
    }

    #[test]
    fn placeholders_take_the_data_and_several_lines_break_the_line() {
        let filled = fill(
            runs("**{name}**\n{lines} end\n{unknown}"),
            &|name| match name {
                "name" => Some("Muster".into()),
                "lines" => Some("a\nb".into()),
                _ => None,
            },
        );

        assert_eq!(texts(&filled), ["Muster", "a", "b end", "{unknown}"]);
        assert!(filled[0][0].bold);
    }

    #[test]
    fn the_builtin_layout_becomes_a_frame_and_parts_for_the_template() {
        let invoice =
            crate::invoice::golden::standard_invoice(crate::invoice::ProfileKind::En16931);
        let view = super::super::view::build(
            &invoice,
            &super::super::view::Branding::default(),
            &crate::layout::Format::default(),
        );
        let layout = crate::layout::builtin();

        let page = page_view(&layout, &view, &mut |_| Ok(String::new())).unwrap();

        assert_eq!(page.margin, layout.page.margins);
        // the first page has a frame of its own, so the content starts below the window
        assert_eq!(
            page.first_margin,
            Some(layout.first.as_ref().unwrap().margins)
        );
        assert!(page.first_parts.as_ref().unwrap().len() > page.parts.len());
        let json = serde_json::to_value(page.first_parts.unwrap()).unwrap();
        let address = json
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "address")
            .unwrap();
        assert_eq!(address["lines"][0], view.buyer.name);
        let infos = json
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "infos")
            .unwrap();
        assert_eq!(infos["rows"][0]["value"], view.facts.number);
    }

    #[test]
    fn a_blurry_image_is_flagged_and_a_missing_one_is_left_out() {
        let invoice =
            crate::invoice::golden::standard_invoice(crate::invoice::ProfileKind::En16931);
        let view = super::super::view::build(
            &invoice,
            &super::super::view::Branding::default(),
            &crate::layout::Format::default(),
        );
        let image = |px| ImageFile {
            name: "logo.png".into(),
            path: "logo.png".into(),
            width_px: px,
            height_px: px,
            vector: false,
        };
        let part = |image| Part {
            anchor: Anchor::TopLeft,
            x: 0.0,
            y: 0.0,
            width: Some(50.0),
            content: PartContent::Image { image },
        };
        let mut staged = |_: &ImageFile| Ok("image-1.png".to_owned());

        // 50 mm are 1.97 inches: 200 pixels are 101 dpi, 400 pixels 203 dpi
        let blurry = part_view(0, &part(Some(image(200))), &view, &mut staged)
            .unwrap()
            .unwrap();
        let sharp = part_view(0, &part(Some(image(400))), &view, &mut staged)
            .unwrap()
            .unwrap();
        let none = part_view(0, &part(None), &view, &mut staged).unwrap();

        let low = |v: &PartView| matches!(v.content, PartViewContent::Image { low_res: true, .. });
        assert!(low(&blurry) && !low(&sharp));
        assert!(none.is_none());
    }

    #[test]
    fn pictures_lie_behind_the_other_parts() {
        let invoice =
            crate::invoice::golden::standard_invoice(crate::invoice::ProfileKind::En16931);
        let view = super::super::view::build(
            &invoice,
            &super::super::view::Branding::default(),
            &crate::layout::Format::default(),
        );
        let at = |content| Part {
            anchor: Anchor::TopLeft,
            x: 0.0,
            y: 0.0,
            width: Some(210.0),
            content,
        };
        let image = ImageFile {
            name: "paper.svg".into(),
            path: "paper.svg".into(),
            width_px: 0,
            height_px: 0,
            vector: true,
        };
        // the picture comes last in the list, the letterhead is added after the rest
        let setup = PageSetup {
            parts: vec![
                at(PartContent::Line {
                    thickness: 0.5,
                    color: None,
                }),
                at(PartContent::Image { image: Some(image) }),
            ],
            ..PageSetup::default()
        };

        let views = setup_parts(&setup, &view, &mut |_| Ok("image-1.svg".into())).unwrap();

        assert!(matches!(views[0].content, PartViewContent::Image { .. }));
        assert!(matches!(views[1].content, PartViewContent::Line { .. }));
    }

    #[test]
    fn a_color_reaches_the_template_and_without_one_the_part_is_as_before() {
        let invoice =
            crate::invoice::golden::standard_invoice(crate::invoice::ProfileKind::En16931);
        let view = super::super::view::build(
            &invoice,
            &super::super::view::Branding::default(),
            &crate::layout::Format::default(),
        );
        let part = |content| Part {
            anchor: Anchor::TopLeft,
            x: 0.0,
            y: 0.0,
            width: Some(50.0),
            content,
        };
        let color = |c: &str| Some(c.to_owned());
        let contents = |color: Option<String>| {
            vec![
                PartContent::Text {
                    text: "Hallo".into(),
                    family: None,
                    size: 9.0,
                    align: Align::Left,
                    color: color.clone(),
                },
                PartContent::Line {
                    thickness: 0.5,
                    color: color.clone(),
                },
                PartContent::PageNumber {
                    text: "{page}".into(),
                    size: 7.5,
                    align: Align::Right,
                    color: color.clone(),
                },
                PartContent::Infos {
                    fields: vec![InfoField::Number],
                    labels: true,
                    align: Align::Left,
                    long_dates: false,
                    size: 9.0,
                    color: color.clone(),
                },
                PartContent::Address {
                    sender_line: true,
                    size: 10.0,
                    color,
                },
            ]
        };
        let json = |color| {
            contents(color)
                .into_iter()
                .map(|content| {
                    let view = part_view(0, &part(content), &view, &mut |_| Ok(String::new()))
                        .unwrap()
                        .unwrap();
                    serde_json::to_value(view).unwrap()
                })
                .collect::<Vec<_>>()
        };

        assert!(json(color("#231f20"))
            .iter()
            .all(|part| part["color"] == "#231f20"));
        // the template tells "no color" by null and keeps the grey of the kind
        assert!(json(None).iter().all(|part| part["color"].is_null()));
    }

    #[test]
    fn sharpness_is_measured_in_dots_per_inch_at_the_placed_size() {
        // 600 pixels over 4 inches (101.6 mm) are 150 dpi
        assert!((dpi(600, 101.6) - 150.0).abs() < 0.01);
        assert!(dpi(300, 101.6) < MIN_DPI);
    }
}
