# Notice

The e-invoice model, business rules and CII/UBL bindings come from the
[`en16931`](https://github.com/hupe1980/en16931) crates (MIT OR Apache-2.0). They implement the
semantic data model of EN 16931-1 and the syntaxes listed in CEN/TS 16931-2. EN 16931-1 and
CEN/TS 16931-2 are made available free of charge by CEN and the European Commission under their
2018 licence agreement, which permits derivative use on condition that derivative applications
carry a statement to this effect. Copyright in the standard remains with CEN.

## Bundled tools

[Typst](https://github.com/typst/typst) (Apache-2.0) renders the invoice PDF; its compiler is linked into the app as a
library, with the fonts it brings itself (Libertinus Serif, New Computer Modern, DejaVu Sans Mono; their licences are in
the NOTICE of the `typst-assets` crate).

The app ships these programs next to its binary and runs them as separate processes:

- [Mustang](https://github.com/ZUGFeRD/mustangproject) (Apache-2.0) attaches the XML and validates the result. Its jar includes
  [veraPDF](https://verapdf.org) (GPL-3.0+ or MPL-2.0, used under the MPL), Saxon and PDFBox with their own licences.
- A Java runtime linked from [Eclipse Temurin](https://adoptium.net) 21 (GPL-2.0 with Classpath Exception), its licence
  files stay in `sidecars/jre/legal`.
- [Open Sans](https://github.com/googlefonts/opensans), SIL Open Font License 1.1, is used as a font file in the
  tests only (`src-tauri/test-fixtures/`, licence next to it), it is not part of the app.
- The two icons (save, remove) are [Font Awesome Free](https://fontawesome.com) solid icons (CC BY 4.0), bundled with the
  app (`src/common/utils/icon.utils.ts`).
