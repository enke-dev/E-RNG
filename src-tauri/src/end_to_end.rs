//! A layout with tab columns on its way through the app, with the real tools: saved like
//! `layout_save` does, shown in the sample (guides on and off), in the preview and as a PDF of a
//! draft, created as an invoice (Typst, combine, validate), exported to a layout file, imported
//! into another store and shown again.
//!
//! `bun run generate:sidecars && cargo test --lib end_to_end -- --ignored`. The files it makes (SVG
//! and PNG of every page, PDFs) go to the folder in `ERNG_E2E_OUT`, else to a temporary one.

use std::path::{Path, PathBuf};

use crate::commands::layouts_command::{render_sample, save};
use crate::commands::preview_invoice_command::shown;
use crate::context::Context;
use crate::creation::{create, Creating};
use crate::draft::fixtures::{draft, settings};
use crate::layout::{builtin, Align, Anchor, CustomLayout, Part, PartContent};
use crate::layout_bundle;
use crate::model::{Kind, Settings};
use crate::render::{self, Branding, PartBox};
use crate::sidecar::Sidecars;
use crate::store::Store;

/// The footer of the layout under test, as it is typed in the part form
const COLUMNS: [&str; 4] = [
    "{seller_name}\n{seller_street}\n{seller_postcode_city}",
    "Telefon:\t{seller_phone}\nMobil:\t{seller_mobile}\nFax:\t{seller_fax}",
    "{seller_email}\n{seller_website}\nSt.-Nr.:\t{seller_tax_number}",
    "{bank_name}\nIBAN:\t{iban}\nBIC:\t{bic}",
];

fn text_part(x: f64, width: f64, text: &str) -> Part {
    Part {
        anchor: Anchor::TopLeft,
        x,
        y: 260.0,
        width: Some(width),
        content: PartContent::Text {
            text: text.into(),
            family: None,
            size: 7.5,
            align: Align::Left,
            color: None,
        },
    }
}

/// The built-in layout with a footer of the four columns above, on every page
fn layout() -> CustomLayout {
    let base = builtin();
    let rule = base.page.parts[0].clone();
    let number = base.page.parts.last().cloned().unwrap();
    let footer: Vec<Part> = std::iter::once(rule)
        .chain(
            COLUMNS
                .iter()
                .zip([25.0, 66.0, 107.0, 148.0])
                .map(|(text, x)| text_part(x, 40.0, text)),
        )
        .chain([number])
        .collect();
    let mut page = base.page.clone();
    let first = base.first.clone().map(|mut first| {
        // the first page: the same footer, then what is its own
        let own = first.parts.split_off(base.page.parts.len());
        first.parts = footer.iter().cloned().chain(own).collect();
        first
    });
    page.parts = footer;
    CustomLayout {
        name: "Spalten".into(),
        page,
        first,
        ..base
    }
}

fn seller(variant: &str) -> Settings {
    let mut settings = settings();
    let seller = &mut settings.seller;
    seller.tax_number = Some("123/456/78901".into());
    seller.mobile = Some("+49 171 5551234".into());
    seller.fax = Some("+49 221 123457".into());
    seller.website = Some("https://muster.example".into());
    seller.bank_name = Some("Musterbank eG".into());
    match variant {
        "no-mobile" => seller.mobile = None,
        "no-fax-tax" => {
            seller.fax = None;
            seller.tax_number = None;
        }
        // what a paste brings along: no-break spaces, a line separator, a zero-width space
        "pasted" => {
            seller.phone = "+49\u{00A0}221\u{00A0}123456".into();
            seller.mobile = Some("0171\u{2028}5551234".into());
            seller.website = Some("https://muster\u{200B}.example".into());
        }
        _ => {}
    }
    settings
}

fn write_pages(out: &Path, name: &str, sidecars: &Sidecars, pages: &[String]) {
    pages.iter().enumerate().for_each(|(i, svg)| {
        let stem = format!("{name}-page-{}", i + 1);
        std::fs::write(out.join(format!("{stem}.svg")), svg).unwrap();
        to_png(sidecars, out, &stem);
    });
}

/// A PNG of an SVG page (the Typst of the app reads SVG, there is no other converter at hand)
fn to_png(sidecars: &Sidecars, out: &Path, stem: &str) {
    let source = format!(
        "#set page(width: 210mm, height: 297mm, margin: 0pt)\n#image(\"{stem}.svg\", width: 100%)\n"
    );
    std::fs::write(out.join(format!("{stem}.typ")), source).unwrap();
    let done = std::process::Command::new(sidecars.typst())
        .current_dir(out)
        .args(["compile", "--format", "png", "--ppi", "160"])
        .arg(format!("{stem}.typ"))
        .arg(format!("{stem}.png"))
        .status()
        .unwrap();
    assert!(done.success());
    std::fs::remove_file(out.join(format!("{stem}.typ"))).unwrap();
}

fn out_dir() -> (PathBuf, Option<tempfile::TempDir>) {
    if let Some(dir) = std::env::var_os("ERNG_E2E_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        (PathBuf::from(dir), None)
    } else {
        let dir = tempfile::tempdir().unwrap();
        (dir.path().to_path_buf(), Some(dir))
    }
}

fn store_with(settings: &Settings) -> (Store, i64) {
    let store = Store::in_memory().unwrap();
    let context = store
        .save_context(&Context::new(settings.clone(), ""), None, None)
        .unwrap()
        .id
        .unwrap();
    (store, context)
}

fn height_of(parts: &[PartBox], index: usize) -> f64 {
    parts
        .iter()
        .find(|part| part.index == index && part.page == 1 && part.first)
        .map_or(0.0, |part| part.h)
}

/// Exports the layout to a file and imports it into a store of its own, with its own files
fn reimport(
    saved: &CustomLayout,
    settings: &Settings,
    out: &Path,
    variant: &str,
) -> (Store, i64, CustomLayout) {
    let files_a = tempfile::tempdir().unwrap();
    let bundle = layout_bundle::export(saved, false, files_a.path()).unwrap();
    std::fs::write(out.join(format!("{variant}.erng-layout")), &bundle).unwrap();
    let (other, other_context) = store_with(settings);
    let files_b = tempfile::tempdir().unwrap();
    let imported =
        layout_bundle::install(layout_bundle::parse(&bundle).unwrap(), None, files_b.path())
            .unwrap();
    let imported = other.store_imported_layout(&imported.layout).unwrap();
    assert_eq!(imported.page, saved.page);
    assert_eq!(imported.first, saved.first);
    (other, other_context, imported)
}

/// The colour of the marks of invisible characters in the SVG of a page
const MARKS: &str = "8e4ec6";

#[test]
#[ignore = "needs the sidecars: bun run generate:sidecars"]
fn tab_columns_go_all_the_way() {
    let sidecars = Sidecars::in_repo();
    let (out, _keep) = out_dir();
    // the same time stamp in every PDF, so that two PDFs of one document compare equal
    std::env::set_var("SOURCE_DATE_EPOCH", "1760000000");

    let mut heights = std::collections::HashMap::new();
    for variant in ["all", "no-mobile", "no-fax-tax", "pasted"] {
        let settings = seller(variant);
        let (store, context) = store_with(&settings);
        let data = tempfile::tempdir().unwrap();

        // saved like `layout_save`
        let saved = save(&store, layout()).unwrap();
        assert!(saved.id.is_some() && !saved.uid.is_empty());

        // the layout sample, guides off and on
        let plain = render_sample(&sidecars, &settings, Some(&saved), false).unwrap();
        let guided = render_sample(&sidecars, &settings, Some(&saved), true).unwrap();
        assert!(plain.problems.is_empty() && guided.problems.is_empty());
        write_pages(&out, &format!("{variant}-sample"), &sidecars, &plain.pages);
        write_pages(
            &out,
            &format!("{variant}-sample-guides"),
            &sidecars,
            &guided.pages,
        );
        assert!(!plain.pages.iter().any(|page| page.contains(MARKS)));
        assert!(guided.pages.iter().any(|page| page.contains(MARKS)));
        heights.insert(
            variant,
            (height_of(&plain.parts, 2), height_of(&plain.parts, 3)),
        );

        // the preview of an invoice draft, and the draft as a PDF
        let mut draft = draft();
        draft.layout = Some(saved.key());
        let (invoice, branding) = shown(&store, &draft, None, context, Kind::Invoice)
            .unwrap()
            .expect("the draft can be built");
        let pages = render::render_preview(&sidecars, data.path(), &invoice, &branding).unwrap();
        assert!(!pages.iter().any(|page| page.contains(MARKS)));
        write_pages(&out, &format!("{variant}-draft-preview"), &sidecars, &pages);
        let pdf = render::render_pdf(&sidecars, data.path(), &invoice, &branding).unwrap();
        std::fs::copy(&pdf, out.join(format!("{variant}-draft.pdf"))).unwrap();

        // a document never gets the marks, whatever the branding asks for
        let guided_branding = Branding {
            guides: true,
            ..branding.clone()
        };
        let again = render::render_pdf(&sidecars, data.path(), &invoice, &guided_branding).unwrap();
        assert_eq!(
            std::fs::read(&pdf).unwrap(),
            std::fs::read(&again).unwrap(),
            "{variant}: the PDF has marks"
        );

        // created: Typst, combine, validate
        let id = store
            .save_draft(None, context, Kind::Invoice, &draft, None)
            .unwrap()
            .id;
        let created = create(&sidecars, &store, &Creating::default(), data.path(), id).unwrap();
        let files = created.invoice.files.unwrap();
        std::fs::copy(
            data.path().join(&files.pdf.path),
            out.join(format!("{variant}-invoice.pdf")),
        )
        .unwrap();
        println!(
            "{variant}: created {:?}, warnings {:?}",
            created.invoice.number, created.warnings
        );

        // exported to a layout file and imported into another store, as on another machine
        let (other, other_context, imported) = reimport(&saved, &settings, &out, variant);

        // and rendered again: the same sample, a created invoice that holds as well
        let again = render_sample(&sidecars, &settings, Some(&imported), false).unwrap();
        assert_eq!(
            again.pages, plain.pages,
            "{variant}: the imported layout looks different"
        );
        draft.layout = Some(imported.key());
        let id = other
            .save_draft(None, other_context, Kind::Invoice, &draft, None)
            .unwrap()
            .id;
        let data = tempfile::tempdir().unwrap();
        let created = create(&sidecars, &other, &Creating::default(), data.path(), id).unwrap();
        std::fs::copy(
            data.path().join(&created.invoice.files.unwrap().pdf.path),
            out.join(format!("{variant}-imported-invoice.pdf")),
        )
        .unwrap();
    }

    // rows without a value are gone: the column is shorter by the line of its label
    let (all, no_mobile, no_fax_tax) =
        (heights["all"], heights["no-mobile"], heights["no-fax-tax"]);
    assert!(no_mobile.0 < all.0, "Mobil stays: {all:?} {no_mobile:?}");
    assert!(
        no_fax_tax.1 < all.1,
        "St.-Nr. stays: {all:?} {no_fax_tax:?}"
    );
    assert!(
        (no_mobile.1 - all.1).abs() < 0.01,
        "an unrelated column changed"
    );
}
