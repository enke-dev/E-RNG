//! What a layout is made of: fonts, and for every page a frame (the margins the flowing
//! content stays inside) with parts around it (information, address, image, text, page number,
//! rule) that are placed by an anchor and an offset. The built-in layout is such a list too.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Distances from the page edge to the content, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for Insets {
    fn default() -> Self {
        Self {
            top: 20.0,
            right: 20.0,
            bottom: 25.0,
            left: 25.0,
        }
    }
}

/// An uploaded font file and the family Typst knows it by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontFile {
    pub family: String,
    /// Weight and style as the font names it (`Bold Italic`), empty when it does not say
    #[serde(default)]
    pub style: String,
    pub path: PathBuf,
}

/// An invoice or offer that is made with an own layout, what keeps the layout from being deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutUsage {
    pub invoice_id: i64,
    pub kind: crate::model::Kind,
    /// `None` for a draft
    pub number: Option<String>,
    pub customer: String,
}

/// One of the nine points of a page or of a part: where a part is attached to the page, and the
/// point of the part that sits there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// A line of the information block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InfoField {
    Number,
    Date,
    /// Offers only
    ValidUntil,
    ServiceDate,
    DueDate,
    BuyerReference,
    PrecedingInvoice,
    TaxNumber,
    VatId,
}

/// A picture of a part, copied into the app's data directory when it was picked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageFile {
    pub name: String,
    pub path: PathBuf,
    /// Pixels of a raster image, they say how sharp it is at its size; 0 for a vector image
    pub width_px: u32,
    pub height_px: u32,
    pub vector: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PartContent {
    /// Date, number and the other facts as rows of label and value
    Infos {
        fields: Vec<InfoField>,
        labels: bool,
        align: Align,
        /// Dates as `18. Oktober 2026` instead of `18.10.2026`
        long_dates: bool,
        size: f64,
    },
    /// The recipient, with the sender in small print above it
    Address {
        sender_line: bool,
        size: f64,
    },
    Image {
        image: Option<ImageFile>,
    },
    /// Bold and italic as in markdown (`**bold**`, `*italic*`), and `{seller_name}`-like
    /// placeholders for the data of the company
    Text {
        text: String,
        /// A font family, none is the font of the layout
        family: Option<String>,
        size: f64,
        align: Align,
    },
    /// `{page}` and `{pages}` stand for the numbers
    PageNumber {
        text: String,
        size: f64,
        align: Align,
    },
    /// A horizontal rule as wide as the part
    Line {
        thickness: f64,
    },
}

/// Something placed on the page. `x` and `y` are millimetres from the anchor towards the inside of
/// the page; at the centre they count to the right and down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub anchor: Anchor,
    pub x: f64,
    pub y: f64,
    /// Millimetres, none lets the content decide
    pub width: Option<f64>,
    #[serde(flatten)]
    pub content: PartContent,
}

/// The frame of the flowing content on a page, and what is placed around it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageSetup {
    pub margins: Insets,
    pub parts: Vec<Part>,
}

/// How numbers, money and dates are written, the German way unless the layout says otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Format {
    /// Between the whole and the fractional part: `,` or `.`
    pub decimal: String,
    /// Between groups of three digits, none for no grouping
    pub thousands: String,
    /// `€` instead of `EUR` where there is a well known symbol
    pub currency_symbol: bool,
    /// `1,00 €` instead of `€ 1,00`
    pub currency_after: bool,
    /// A (non-breaking) space between amount and currency
    pub currency_space: bool,
    /// `19 %` instead of `19%`
    pub percent_space: bool,
    /// `−` instead of `-` for deductions
    pub true_minus: bool,
    /// `DE02 1203 …` instead of `DE021203…`
    pub iban_groups: bool,
    /// A pattern like dayjs has them: `DD.MM.YYYY`
    pub date: String,
    /// The same for the written out date (infos part): `D. MMMM YYYY`
    pub date_long: String,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            decimal: ",".into(),
            thousands: ".".into(),
            currency_symbol: true,
            currency_after: true,
            currency_space: true,
            percent_space: true,
            true_minus: false,
            iban_groups: false,
            date: "DD.MM.YYYY".into(),
            date_long: "D. MMMM YYYY".into(),
        }
    }
}

/// A layout the user made. It starts as the built-in one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomLayout {
    pub id: Option<i64>,
    /// The context it belongs to, none: every context can use it. Stored beside the layout.
    #[serde(default)]
    pub context_id: Option<i64>,
    /// Stays with the layout through renaming, export and import: tells copies of one layout from
    /// layouts that only share a name. Made when the layout is first stored.
    #[serde(default)]
    pub uid: String,
    /// When it was last saved (UTC, `2026-10-20T08:15:00Z`), the store sets it
    #[serde(default)]
    pub updated_at: String,
    pub name: String,
    /// The pages after the first, and the first unless it has its own
    #[serde(default)]
    pub page: PageSetup,
    /// The first page with a frame and parts of its own, e.g. for a letterhead
    #[serde(default)]
    pub first: Option<PageSetup>,
    /// Font files, the family of the first one is the font of the layout unless `font` names another
    pub fonts: Vec<FontFile>,
    #[serde(default)]
    pub font: Option<String>,
    /// How numbers, money and dates are written
    #[serde(default)]
    pub format: Format,
    /// Out of the selection, kept for the invoices that were made with it
    #[serde(default)]
    pub archived: bool,
}

impl CustomLayout {
    /// The id the settings and invoices refer to it by
    pub fn key(&self) -> String {
        format!("custom-{}", self.id.unwrap_or_default())
    }

    /// The family the content is set in, if the layout brings fonts
    pub fn family(&self) -> Option<&str> {
        self.font
            .as_deref()
            .or_else(|| self.fonts.first().map(|f| f.family.as_str()))
    }
}

fn text(anchor: Anchor, x: f64, y: f64, width: f64, text: &str, size: f64, align: Align) -> Part {
    Part {
        anchor,
        x,
        y,
        width: Some(width),
        content: PartContent::Text {
            text: text.into(),
            family: None,
            size,
            align,
        },
    }
}

/// The footer every page of the built-in layout has: a rule, three columns of company data and a
/// free text under it, and the page number. They hang from the top so the columns start on the
/// same line (the paper is A4). The text under the columns has room for four lines inside the area a
/// desktop printer prints (5 mm from the edge).
fn footer() -> Vec<Part> {
    use Anchor::{TopLeft, TopRight};
    vec![
        Part {
            anchor: TopLeft,
            x: 25.0,
            y: 263.0,
            width: Some(165.0),
            content: PartContent::Line { thickness: 0.5 },
        },
        text(
            TopLeft,
            25.0,
            266.0,
            53.0,
            "**{seller_name}**\n{seller_address}",
            7.5,
            Align::Left,
        ),
        text(
            TopLeft,
            81.0,
            266.0,
            53.0,
            "{seller_contact}",
            7.5,
            Align::Left,
        ),
        text(
            TopLeft,
            137.0,
            266.0,
            53.0,
            "{seller_registration}",
            7.5,
            Align::Left,
        ),
        // room for four lines inside the printable area, narrower than the rule so the page number
        // stays clear
        text(TopLeft, 25.0, 278.0, 140.0, "{legal}", 7.5, Align::Left),
        Part {
            anchor: TopRight,
            x: 20.0,
            y: 278.0,
            width: Some(60.0),
            content: PartContent::PageNumber {
                text: "Seite {page} von {pages}".into(),
                size: 7.5,
                align: Align::Right,
            },
        },
    ]
}

/// The built-in layout: a window letter after DIN 5008 form B. The first page carries the company
/// name, the address in the window and the facts; the pages after it only the footer.
pub fn builtin() -> CustomLayout {
    use Anchor::TopLeft;
    let margins = |top| Insets {
        top,
        right: 20.0,
        bottom: 36.0,
        left: 25.0,
    };
    let mut first = footer();
    first.extend([
        text(
            TopLeft,
            25.0,
            20.0,
            120.0,
            "**{seller_name}**",
            14.0,
            Align::Left,
        ),
        Part {
            // inside the window (20 to 105 mm), but not left of the frame
            anchor: TopLeft,
            x: 25.0,
            y: 50.0,
            width: Some(80.0),
            content: PartContent::Address {
                sender_line: true,
                size: 10.0,
            },
        },
        Part {
            anchor: Anchor::TopRight,
            x: 20.0,
            y: 50.0,
            width: Some(75.0),
            content: PartContent::Infos {
                fields: vec![
                    InfoField::Number,
                    InfoField::Date,
                    InfoField::ValidUntil,
                    InfoField::ServiceDate,
                    InfoField::DueDate,
                    InfoField::BuyerReference,
                    InfoField::PrecedingInvoice,
                ],
                labels: true,
                align: Align::Right,
                long_dates: false,
                size: 9.0,
            },
        },
    ]);
    CustomLayout {
        id: None,
        context_id: None,
        uid: String::new(),
        updated_at: String::new(),
        name: "Klassisch".into(),
        page: PageSetup {
            margins: margins(20.0),
            parts: footer(),
        },
        first: Some(PageSetup {
            margins: margins(98.0),
            parts: first,
        }),
        fonts: Vec::new(),
        font: None,
        format: Format::default(),
        archived: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_layout_survives_a_round_trip_through_json() {
        let layout = builtin();
        let json = serde_json::to_string(&layout).unwrap();
        assert_eq!(serde_json::from_str::<CustomLayout>(&json).unwrap(), layout);
    }

    #[test]
    fn a_layout_stored_before_parts_existed_loads_with_an_empty_frame() {
        let old = r#"{"id":1,"name":"Alt","background":null,"margins":{"top":1,"right":1,"bottom":1,"left":1},
            "first_margins":null,"fonts":[]}"#;
        let layout: CustomLayout = serde_json::from_str(old).unwrap();
        assert_eq!(layout.page.parts, Vec::new());
        assert!(layout.first.is_none());
    }

    #[test]
    fn parts_travel_flat_with_their_kind() {
        let json = serde_json::to_value(&builtin().page.parts[1]).unwrap();
        assert_eq!(json["kind"], "text");
        assert_eq!(json["anchor"], "top-left");
        assert_eq!(json["size"], 7.5);
    }
}
