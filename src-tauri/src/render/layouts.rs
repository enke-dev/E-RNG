//! The design of an invoice document: one Typst file that lays out the `data.json` the view builds
//! (see `view.rs`). It is the only built-in layout, users make their own from it with stationery,
//! margins and a font (`CustomLayout`).

use serde::Serialize;

use crate::model::CustomLayout;

/// The id of the built-in layout, what settings and invoices say when they mean "no own layout"
pub const STANDARD: &str = "standard";

pub const STANDARD_SOURCE: &str = include_str!("layouts/standard.typ");

/// What the helpers the layout imports bring: page margins, stationery and fonts of an own layout.
pub const PRELUDE: &str = include_str!("layouts/prelude.typ");

#[derive(Debug, Serialize)]
pub struct LayoutInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Made by the user, not built in
    pub custom: bool,
    /// Out of the selection, kept for the invoices that were made with it
    pub archived: bool,
}

/// The built-in layout followed by the user's own.
pub fn all(customs: &[CustomLayout]) -> Vec<LayoutInfo> {
    std::iter::once(LayoutInfo {
        id: STANDARD.into(),
        name: "Klassisch".into(),
        description: "Serifenschrift, Positionen mit Menge, Einheit und Preisen, Zahlungsblock"
            .into(),
        custom: false,
        archived: false,
    })
    .chain(customs.iter().map(|c| LayoutInfo {
        id: c.key(),
        name: c.name.clone(),
        description: "Eigenes Layout".into(),
        custom: true,
        archived: c.archived,
    }))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_layout_comes_first() {
        assert_eq!(all(&[])[0].id, STANDARD);
        assert_eq!(all(&[]).len(), 1);
    }
}
