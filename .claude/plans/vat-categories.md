# VAT categories and invoicing abroad – Plan

Goal: invoice customers abroad and tax-free sales correctly: reverse charge, intra-community deliveries, exports, exempt or not taxable services, each with the reason the e-invoice needs.

Status: idea, not started. Builds on the rates per context and position (label and percent, see [docs/development.md](../../docs/development.md)). Every rate is category S today.

## Model

A rate entry becomes label, category and percent, like a unit is label and code:

- Category from UNCL5305 (the code list of EN 16931, BT-151 per line, BT-118 per breakdown):
  - `S` standard (19 %, 7 %), the only one with a percent above 0
  - `Z` zero rated
  - `E` exempt, e.g. §4 UStG
  - `AE` reverse charge (§13b UStG), e.g. services to a business in another EU country
  - `K` intra-community supply of goods (§4 Nr. 1b, §6a UStG)
  - `G` export outside the EU (§4 Nr. 1a, §6 UStG)
  - `O` not subject to VAT, e.g. services whose place of supply is abroad outside the EU
  - `L`, `M` (Canary Islands, Ceuta and Melilla) only if ever needed
- Every category but `S` has 0 % and an exemption reason (BT-120 text and/or BT-121 code from the CEF VATEX list, e.g. `VATEX-EU-AE`, `VATEX-EU-IC`, `VATEX-EU-G`, `VATEX-EU-O`). The entry carries a default reason text in German that the user can change.
- Defaults stay „Regelsatz“ (S, 19 %) and „Ermäßigt“ (S, 7 %); entries for the other categories are offered as presets when adding, not seeded.
- A position stores category and reason with rate and label, frozen like today.

## Rules to enforce

- `AE` and `K`: the buyer's VAT id (BT-48) and the seller's VAT id are required (the `BR-AE-*` and `BR-IC-*` rules); `K` also wants the delivery date or period and the deliver-to country.
- `G`: deliver-to country outside the EU.
- One breakdown (BG-23) per category and rate; categories cannot be mixed on one line.
- Mixing `S` lines with `AE` lines on one invoice is allowed; check what the Schematron rules say about mixing `O` with others (BR-O-* forbid it).
- The customer needs a country other than DE to be offered `AE`, `K`, `G` (warn, do not block).
- Kleinunternehmer stays the context switch „Keine Umsatzsteuer“ (E with the §19 reason for the whole document) and hides all of this.

## Document

- The positions column `USt.` shows the label or the category for 0 % lines („Reverse Charge“ instead of „0 %“).
- Each non-`S` breakdown prints its reason under the totals (the statement „Steuerschuldnerschaft des Leistungsempfängers“ for `AE` is mandatory on the PDF too, §14a Abs. 5 UStG).
- English wording for customers abroad is its own item (see Open work: English invoices).

## Checks

- Golden XML tests per category validated with Mustang and veraPDF (XRechnung and ZUGFeRD EN 16931 profile).
- The Steuerberater export (see `steuerberater-export.md`) sums per category and rate; DATEV needs a revenue account per category.
