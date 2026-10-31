# Vorgänge – Plan

**Status (2026-11-01): phases 1–5 done, not pushed yet; phase 6 next.**
Details of what is built are in `docs/development.md`. A living plan: decisions are edited in place,
what is dropped is removed, not kept as history.

## Idea

A **Vorgang** wraps the documents of one business case for one customer: offers, invoices and the drafts
traded with the customer before a document is baked. It has its own number and, later, a timeline built
from the history of its documents. Users who only want offers and invoices never have to see it.

## Model

- **Every document is in exactly one Vorgang.** Creating a plain offer or invoice creates its Vorgang
  with it, unseen. So there is one model, no "document without Vorgang" case, and any document's
  Vorgang can be picked up later to add more to it.
- **A Vorgang has**: context, number, customer, an optional internal title (`Badsanierung`), created at.
  Its status is not stored, it is derived from its documents (offered, accepted, invoiced, open
  amount, paid completely), archived when all its documents are.
- **Starting one**: `Neuer Vorgang` asks for the customer and makes an empty Vorgang; documents are
  added to it. A plain new offer or invoice makes its Vorgang with it. Deleting a document that leaves
  its Vorgang empty deletes the Vorgang too (a gap in the numbers, which is fine); one made empty by
  `Neuer Vorgang` stays until deleted by hand.
- **The customer belongs to the Vorgang**, its documents take it from there.
  - While the Vorgang holds a single document that is not baked, the customer can be changed (as today
    on a draft), which changes the Vorgang's customer.
  - Once a second document is added or the first is baked, the customer is locked for the Vorgang and
    all documents that follow. A Vorgang started with `Neuer Vorgang` is locked for its customer from
    the start. The lock is derived from the data (how it was started, its documents), not a flag.
  - A document with another customer than its Vorgang (a parent company paying) is not possible for
    now, the model leaves room for it later.
- **Numbering**: every Vorgang gets a number at once, unseen ones too; whether it is ever used is not
  known when it is made. Own pattern and reset in a settings section per context, the same machinery
  as document numbers (`numbering.rs`), default `[VG-]YYYY-NNN` with the fiscal-year reset, the same
  examples and counter (`Zähler ändern…`) as the documents. Vorgang numbers need not be gapless. On a document's layout the number is a placeholder
  (`{vorgang_number}`), opt-in by the user; not in the XML for now (BT-11 Projektreferenz would fit).
- **Contexts**: a Vorgang belongs to one, like its documents. Backup and restore and deleting a
  context cover the new tables.

## Documents: drafts, versions, baking

- **Version** of a document = the count of its own draft exports so far (`Exportieren als PDF` of a
  draft, sent to the customer to agree on). Not stored, derived from the export events. Same behaviour
  for offers and invoices.
- **Titles with placeholders**: the layout's format gets title patterns, one for drafts and one for
  baked documents, with placeholders: `{title}` and `{draft_title}` (the words of the kind,
  `Wording`), `{number}`, `{version}`, `{vorgang_number}`. Defaults give today's titles (`{title}`,
  `{draft_title}`); `Angebotsentwurf · Version {version}` or `Ihre {title} {number}` are a setting
  away. `{version}` and `{vorgang_number}` work in text parts too.
- **Placeholders everywhere they make sense**: text parts, the page number part, the headline
  patterns and the letter texts (intro and closing of every kind, settings and document; they take
  what text parts take, plus `{number}`, `{date}`, `{due_date}`, `{customer_name}`). Each such field
  has a round outlined button that lists its placeholders, grouped, searchable, with meaning and
  example, and inserts the one picked at the cursor. Not in the note (it goes into the XML as it is)
  or in positions.
- **Baking** is the final export: it assigns the document number (as today) and happens once. After it
  the document is fixed (GoBD for invoices).
- Each draft export keeps a **snapshot of the document's data** (not the PDF), so versions can be
  compared. The PDF is only kept for the baked document, as today.

## History

- **Events**, append-only, per document: created (from what, for copies and invoices from offers),
  draft exported (version n), baked, payment received and taken back, accepted, declined, answer
  taken back, archived, restored. Documents never move between Vorgänge, so there is no event for
  it. No event for edits (autosave); what changed shows in the diff of two versions. Undoing a mark (a payment, accepted) is an event of its
  own. Each event keeps the time it was recorded and, where it has one, the date it is about (paid on
  a day in the past, a backdated invoice); the timeline is in the order recorded, shown latest first
  with a dashed line above that runs on to now, and shows that date with each event. In a Vorgang an
  entry names the kind (`Angebot angelegt`) and shows the document's number as a tag. The backbone of
  every history view.
- **Payments**: an invoice gets payments (amount, date) instead of its single paid date. Paid
  completely when they add up to its gross total; the open amount is the rest. Each payment is an
  event. `Zahlung erfassen` proposes the open amount and today and takes no more than is open (the
  store refuses it too); less is a part payment. No paid switch. The Schlussrechnung later deducts what was
  paid on the Abschläge.
- **Snapshots** only when a document is created, at draft exports and at baking, never on autosave. A diff compares two snapshots.
- The Vorgang's timeline is the merged history of its documents; a document's own history (detail
  view, later) reuses the same view.

## UI

Reuse what exists, the looks are not decided yet.

- **Navigation**: while this plan is built, "Vorgänge" is a new main entry before "Angebote" and
  "Rechnungen". When it is done, both may be dropped in favour of "Vorgänge" alone.
- **Vorgänge list**: the sidebar list with inset items, as in the settings. Every Vorgang is a group,
  the single-document ones too (to be revisited with the UI follow-ups): number, customer and the
  internal title last as the header, which opens the Vorgang page; its documents inset below by issue
  date (from phase 6 offers before the
  invoices billing them), an action to add an offer or invoice. Filterable by kind, status and
  customer.
- **Document history**: a vertical timeline as the last card of the document view (after the
  timelines of estino-ui or the kvlm website).
- **Duplicating** asks every time: "Neuer Vorgang" (preselected, a copy as today; the customer may be
  changed) or "In diesem Vorgang" (customer locked). From phase 8 a new one is a Folgevorgang.

## Relations

Two levels. Most relations are carried by **positions**: an invoice position may refer to an offer
position (or to a whole offer) with a share, a quantity or a percentage. Which documents an invoice
bills follows from its positions, there is no link table. Only what has no position to hang on is a
**document** relation. The offer ↔ invoice links of today go.

### Offer → offer (document)

| Case                        | Meaning                                                            | Vorgang             |
| --------------------------- | ------------------------------------------------------------------ | ------------------- |
| Angebotsversion             | the drafts traded before baking, see versions; no relation         | same document       |
| Nachtragsangebot (Nachtrag) | more work in the same case, refers to the main offer               | same                |
| Alternativangebot           | a variant of another offer, one of them is taken, the others lapse | same                |
| Ersatzangebot               | replaces a baked offer (rare, agreeing happens in drafts)          | same                |
| Folgeangebot                | a follow-up deal based on an earlier one (next year's maintenance) | new, a Folgevorgang |

How they are made: `Angebot dazu` on a created offer makes the new offer in its Vorgang, referring
to it for good; only created offers are referred to, so the number on the document is fixed. A
Nachtrag starts blank with the customer, an alternative and an Ersatzangebot as a copy. A Nachtrag
to a Nachtrag and an alternative to an alternative refer to what theirs refers to (one main offer,
one group). The document names the offer in its information block (`Nachtrag zu Angebot AN-…`,
`Alternative zu …`, `Ersetzt …`). A declined offer gets no Nachtrag, an accepted one no
replacement, a group with an accepted offer no further alternative.

**Offer states**: a baked offer is marked **accepted** or **declined** with a date, as an invoice is
marked paid; each is a history event. The rest is derived, never stored: an Alternativangebot lapses
when another of its group is accepted, an Ersatzangebot supersedes the one it replaces, an offer past
`valid_until` without answer is expired. Lapsed (`Entfallen`) and superseded (`Ersetzt`) offers are
not accepted or billed and do not expire. The Vorgang's status is derived from these marks, it is
not marked itself: lapsed and replaced offers are no longer offered, an alternative group is offered
once, by its head, until one of it is accepted. No Auftragsbestätigung as a document kind: in
German practice the seller sends it to confirm an order, but here the accepted date covers it; a
printable one could come later.

### Offer → invoice (positions)

| Case               | Refers to                                                  | Final?             |
| ------------------ | ---------------------------------------------------------- | ------------------ |
| Rechnung           | all positions of an offer, in full                         | yes                |
| Teilrechnung       | some positions or quantities, a finished part of the work  | yes, for those     |
| Abschlagsrechnung  | progress so far: a percentage of the offer or of positions | no, deducted later |
| Anzahlungsrechnung | before any work, mostly a percentage                       | no, deducted later |
| Schlussrechnung    | the whole offer, minus the Abschläge and Anzahlungen paid  | yes                |
| Sammelrechnung     | positions of several offers in one invoice                 | yes                |

**Anzahlung and Abschlag are told apart by the work, deterministically**:

- An **Anzahlungsrechnung** asks for money before any work is done; nothing happens until it is paid.
  It has no Leistungszeitraum: § 14 Abs. 4 Nr. 6 UStG asks for the date of the service or, for a
  payment before it, the date the payment is received if it is known and differs from the invoice
  date. Type code 386 (prepayment invoice).
- An **Abschlagsrechnung** bills the work done so far: everything up to now, or a period, which is its
  Leistungszeitraum (required). Type code 326 (partial invoice), in construction 875.
- VAT is due on receipt for both (§ 13 Abs. 1 Nr. 1 a Satz 4 UStG), both are deducted by the
  Schlussrechnung. Verify that Mustang and the XRechnung code list accept 386, 326 and 875.
- In the XML the Schlussrechnung likely states its full amounts, refers to the Abschläge as preceding
  invoices (BG-3) and puts what was paid on them in BT-113 (paid amount), instead of negative lines;
  verify against EN 16931 and XRechnung before phase 7.

"Final" decides the open amount: a final position bills its part for good, an Abschlag or Anzahlung
is only a payment on account until the Schlussrechnung settles it. So the difference between Abschlag
and Teilrechnung is not percentage versus positions (an Abschlag can be 50 % of one position), it is
provisional versus final.

### Invoice → invoice (document)

| Case                                              | Meaning                                            |
| ------------------------------------------------- | -------------------------------------------------- |
| Schlussrechnung → Abschlags-/Anzahlungsrechnungen | deducts them, each with its VAT (§ 14 Abs. 5 UStG) |
| Rechnungskorrektur / Stornorechnung → invoice     | corrects or cancels it (BT-25)                     |
| Ersatzrechnung → stornierte Rechnung              | the invoice that replaces a cancelled one          |

### Vorgang → Vorgang

Vorgänge relate, they never merge (two numbers, two locked customers, two histories cannot become
one). A Sammelrechnung over several Vorgänge is a new Vorgang referring to its sources; a Folgeangebot
starts a Folgevorgang referring to the original. Directed (source → follower), every document stays in
exactly one Vorgang. Built late, cheap if the derived status goes through one function from the
start that can then also follow these relations.

## Out of scope for now

- A document with a customer other than its Vorgang's.

## Existing data is migrated

Cheaper than dropping it: a dropped schema still needs old databases detected and wiped or refused,
plus the releases cleaned up, and the test data on the Mac and the Windows machine would be gone. The
migrations are small (see phases 2, 3 and 6), each a function with tests in `store.rs` like the
others.

## Phases (each ships on its own, in this order)

1. **Done. Vorgänge list** as a main entry: for now each document is its own group, with filters. No new
   data.
2. **Done. History**: events and draft-export snapshots per document, the timeline card on the document.
   Payments instead of the paid date. Offers marked accepted or declined with a date. Title patterns
   and `{version}`. Migration: events made up from the stored dates (exported, archived), a paid date
   becomes one payment of the gross total; no snapshots for the past.
3. **Done. Vorgang**: table, numbering settings, the customer rules, unseen Vorgänge for plain documents,
   `Neuer Vorgang`, creating a document inside one, the groups in the list, the timeline,
   `{vorgang_number}`, duplicating with its question. The derived status
   (offered, accepted, invoiced, open, paid) through one function. Migration: one Vorgang per group of
   linked documents (union over the links, split where the customers differ), numbered per context in
   the order of the issue dates.
4. **Done. Versions compared**: a compare button on each kept version in the history (with the next
   newer one, the newest of a draft with the draft now, only where they differ), any two versions
   picked in the dialog, the fields, positions and totals that differ.
5. **Done. Offer → offer**: Nachtrag, Alternativangebot (lapsing), Ersatzangebot (superseding),
   `Angebot dazu`, the row on the document, stages and tags. Schema 14, no data to migrate.
6. **Next. Position references, final**: Rechnung (today's "invoice from offer"), Teilrechnung,
   Sammelrechnung within a Vorgang; the open amount per offer and position. The offer ↔ invoice
   links go (their grouping lives on in the Vorgang since phase 3, the printed offer numbers in
   `frozen_offers`). An invoice that bills an offer can say so in its texts, a placeholder like
   `{offer_number}` for "Laut Angebot AN-2026-003", by its kind or relation.
7. **On account**: Anzahlungs- and Abschlagsrechnung, the Schlussrechnung deducting them.
8. **Vorgang → Vorgang**: Folgeangebot and Folgevorgang, Sammelrechnung across Vorgänge.
9. **Corrections**: Rechnungskorrektur, Stornorechnung, Ersatzrechnung (`to_credit_note` exists).
