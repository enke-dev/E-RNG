# PDF stationery – Plan

## Review decisions (2026-10-29) – they override the text below

- **No pictures**: a letterhead page that would break PDF/A (fonts not embedded, mostly) is not rasterized and not refused outright. Its text is drawn with a fallback font and stays vector: the page is converted once at intake (text with unembedded fonts becomes outlines drawn from a bundled fallback font, everything else is copied), the converted page is what the layout keeps, and it passes the same PDF/A check. Open: which offline tool does this conversion (to be researched when this plan is started: a Rust PDF crate such as `hayro`/`krilla`, or a bundled CLI; Ghostscript is AGPL and heavy). Only if no font can stand in (an unknown symbol font) the page is refused with the findings.
- One page per part is enough (open question 2 closed).

Goal: a letterhead that exists as a PDF (what a print shop or designer hands out) can be used in a layout as it is, sharp at any zoom, instead of being converted to PNG first. First use: derive a layout once from the PDF stationery of a neighbour.

Status: planned 2026-10-29, not started. Related: the "Layouts" section of [docs/development.md](../../docs/development.md) (image parts, `layout_files.rs`, `layout_bundle.rs`).

## How it works today (for reference)

- A letterhead is an image part as wide as the page (210 mm at 0/0), one per page setup; pictures lie behind the other parts.
- `layout_files::store_image` takes PNG, JPG or SVG up to 5 MB, measures raster pixels (`ImageFile.width_px/height_px`, `vector`), and the sample flags rasters under 150 dpi.
- `prelude.typ` places a picture with `image(part.file, width: 100%)`.
- Exported layouts carry their pictures as base64 under made-up names (`layout_bundle.rs`), imported ones pass the same intake.

## Decisions

- **PDF is one more image format, not a new part kind.** It goes through the same picker, intake, bundle export/import and pruning as PNG/JPG/SVG. The Typst version we bundle (0.15.1) places PDF pages as images (`image("x.pdf", page: n)`), so nothing is converted.
- **One page per part.** The part gets a `page` (from 1, default 1). A two-page stationery (first page with the full letterhead, following pages with a slim header) becomes two parts: page 1 in the first-page setup, page 2 in the setup of the other pages. The form offers the page only when the file has more than one.
- **Page count and size are read on intake**, so the form can offer pages and the sample can size the part: `lopdf` (pure Rust, already planned for the historic import) reads the page tree and the MediaBox/CropBox of each page. A file that is encrypted, damaged or has no pages is refused at intake with a German message, like an unreadable picture today.
- **Treated as vector**: no dpi warning. Raster images inside the PDF are the designer's business; the sample cannot tell.
- **Limit stays 5 MB** per file; stationery PDFs are usually far below. Raise it only if a real file needs it.
- **PDF/A is the risk.** The created invoice is PDF/A-3b, checked by veraPDF through Mustang. Typst re-writes an embedded PDF page into its own output, but a source page with unembedded fonts or certain transparency/colour setups may still fail PDF/A. So:
  - On intake, render a one-page test PDF with the page in PDF/A-3b mode and run it through the same validation (`package.rs`) as a created invoice. A page that does not pass is refused with "Diese PDF-Seite lässt sich nicht in eine PDF/A-Rechnung übernehmen" and the veraPDF findings; the user can still convert it to PNG/SVG outside the app.
  - The intake check needs the sidecars (Typst, Java), so it runs async like the layout sample.

## Data model

- `ImageFile` gains `page_count: u32` (`#[serde(default)]`, 0 for non-PDF) and `pdf: bool` (`#[serde(default)]`); `width_px/height_px` stay 0 for PDF, the size is `width_mm/height_mm` of the chosen page (new optional fields, also handy for SVG later).
- The image part gains `page: Option<u32>` (`#[serde(default)]`, none = 1).
- No migration: layouts are JSON, old ones read with the defaults.

## Backend (src-tauri)

- `layout_files.rs`: `IMAGE_EXTENSIONS` + `pdf`; `store_image` detects `%PDF-`, reads page count and page sizes (`lopdf`), sets `vector`/`pdf`; pruning keeps `.pdf` like the other pictures.
- `layout_files.rs` (or a small `stationery.rs`): `check_pdf_page(sidecars, file, page) -> Result<(), Vec<Issue>>` renders `#set page(width: auto, height: auto, margin: 0pt); #image("x.pdf", page: n)` with `--pdf-standard a-3b` and validates it.
- `render/parts.rs`: pass `page` into the part view; `prelude.typ`: `image(part.file, page: part.page, width: 100%)` (Typst ignores `page` for non-PDF? *verify*; otherwise only pass it for PDFs).
- `layout_bundle.rs`: nothing beyond the extension list, files travel as bytes already; intake on import also runs the PDF/A check.
- `commands/layouts.command.rs`: the image picker filter gets `pdf`; a command `layout_check_pdf_page` the form calls when page or file changes.

## Frontend (src)

- `part-form.component.ts`: accept `.pdf`; when the image is a PDF with more than one page, a `Seite` select (1…n); show the check result as a callout under the image ("Seite 2 passt" is silent, a refusal shows the findings).
- The layout sample needs nothing new.

## Tests

- Intake: a one-page and a two-page fixture PDF (made with Typst in the test, so no binary fixture), page count and sizes; an encrypted or broken file is refused; `.pdf` survives bundle export/import and pruning.
- Ignored (sidecars): a layout with a PDF letterhead creates a valid invoice (golden case `stationery-pdf`), page 2 on following pages; a PDF page with an unembedded font is refused by the check.

## Manual checks

- The neighbour's PDF: pick it, page 1 on the first page, page 2 (if any) on the others, frame margins so the content clears the letterhead, guides on; create an invoice and open it in a PDF viewer at 400 %.
- Export the layout with and without fonts, import it on a fresh dev data folder.

## Later

- Use the same intake for a PDF background behind the whole page (not just a part) if the part model turns out to be awkward for that.
- Show thumbnails of the pages in the select.

## Open questions

1. When a stationery page fails the PDF/A check: refuse (proposed), or offer to rasterize it at 300 dpi automatically?
2. Is one page per part enough, or does any stationery need different pages per invoice page beyond "first" and "others"?
