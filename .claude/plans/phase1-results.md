# Phase 1 spike results (2026-10-17, macOS arm64)

Tools: Typst 0.15.1, Mustang CLI 2.26.0, veraPDF 1.30.3 (standalone), KoSIT validator 1.6.3 + XRechnung config 2026-08-31, `page` 0.11.0, Temurin JDK 21.0.12.
Spike code and tools were deleted after the run; commands are in the plan.

## Pipeline

`typst compile --pdf-standard a-3b` (plain, no attachment) -> `mustang --action combine --format fx --version 2 --profile E` -> `mustang --action validate`

| File | Mustang | veraPDF 3b | page 3b |
| --- | --- | --- | --- |
| Typst plain (no XML) | invalid (Factur-X XMP missing, 8 errors) | pass | pass |
| Typst + own `pdf.attach` | invalid (same 8 XMP errors) | pass | pass |
| Typst + `combine` | **valid** (PDF 3u, XML, overall) | pass | fails 3b only, XMP says U; passes with declared profile 3u |
| combine on file with own attach | valid, `factur-x.xml` appears twice in file (not used) | - | - |
| 4 corpus files (180 lines/10 pages, 2 pages long text, logo + glyphs, minimal), combined | all valid | all pass | all conformant |
| XML round trip (`extract`) | byte-identical | | |

## Mutations

| Mutation | Mustang | KoSIT | veraPDF | page |
| --- | --- | --- | --- | --- |
| Grand total changed (BR-CO-15/16) | invalid | reject | n/a | n/a |
| Factur-X XMP missing | invalid | n/a | pass | pass |
| `/AFRelationship` Alternative -> Supplement | **valid** | n/a | pass | pass |
| Not PDF/A at all | not run | n/a | not run | non-conformant |

## Sidecar

- Mustang jar 59 MB (26.9k classes, bundles veraPDF **1.30.2**, Saxon, PDFBox; XRechnung and Peppol rules included)
- `jdeps` modules: java.base, compiler, desktop, instrument, management, naming, sql, jdk.xml.dom (+ xml, logging, prefs, unsupported added)
- `jlink` image: **51 MB** with `--compress zip-9`; jar runs on it
- Time per validate: **1.4 s** (full JDK) / **2.1 s** (jlink image, includes JVM start, cold file cache); `page`: 3 ms
- Exit code: 0 valid, 255 invalid or missing file; report is XML on stdout

## Findings

1. Typst PDF/A-3b + Mustang `combine` works without any lopdf/XMP code. Typst's `pdf.attach` is not needed.
2. Mustang changes XMP conformance B -> U (A-3u). Fine for ZUGFeRD, but a validator forced to `3b` complains.
3. A PDF/A validator alone cannot gate: veraPDF and `page` pass files that are not valid ZUGFeRD.
4. Nobody validates the attachment relationship; only `combine` guarantees it. Do not touch the file after combine.
5. Mustang 2.26.0 ships veraPDF 1.30.2 (advisory GHSA-5gvp-6g43-g47v: XML hardening inert, fixed in 1.30.3). Mustang master already uses 1.30.3, so wait for the next release or build from master.
6. Mustang runs EN 16931 (ZF 2.5), XRechnung 3.0 (BR-DE-*) and Peppol rules; KoSIT agrees on the one XML mutation tried.
7. `page` agrees with veraPDF on every file tried, but it is no gate (see 3).

## Not done / open

- Untrusted-input behavior (XXE, network access) of the sidecar not tested; input here is our own output
- Unembedded-font mutation not tested (Typst always embeds)
- `page`'s own veraPDF corpus gate not run
- KoSIT only on one XML; XRechnung-profile sample (Leitweg-ID) not yet run
- `combine` for profile X (XRechnung) not yet tried
- Windows/Intel not relevant (macOS arm64 only)
