# UI follow-ups – Plan

Goal: more room and calmer views once the groundwork of [vorgaenge.md](vorgaenge.md) is in, plus app
settings that belong to the app, not to a context. A living plan: what is built is summed up here,
the details are in `docs/development.md`.

Status (2026-11-03): the Vorgänge as the only list, the document graph and the Vorgang stages are
built, not pushed yet; the rest is idea.

## Done: the Vorgänge are the list, the Vorgang shows its documents

- **Menu**: Vorgänge, Kunden, Positionen, Einstellungen. The lists of offers and invoices are gone,
  their old addresses (`/`, `/offers/7`, `/invoices/7`) lead into the Vorgänge. `Neu` there is
  `Neuer Vorgang` or `Sammelrechnung`; documents are started on the Vorgang page. An overview of all
  open invoices may come back as a filter of the Vorgänge if it is missed.
- **Sidebar list**: only Vorgänge, grouped by customer (the heading names the customer once). An
  item: its name as the title, the sum, the VG number and day below, the stage tag. Filters by
  stage, year, customer and context. An address naming a Vorgang or document that is gone falls back
  to the list without an error.
- **Every Vorgang has a name**: asked for by `Neuer Vorgang` and the Sammelrechnung, never blank;
  copies and Folgeangebote take the original's, older data is named after its first document.
- **Stage tags say the Vorgang's furthest step**, not a document's: `Neu`, `Angebot in Arbeit`,
  `Rechnung in Arbeit`, `Angebot erstellt`, `Angebot angenommen`, `Angebot abgelehnt`, `Rechnung
  gestellt`, `Teilweise bezahlt`, `Überfällig`, `Teilweise berechnet`, `Bezahlt`, `Storniert`. While
  the customer is to pay, the payment tells (overdue past a due day first); once paid, whether
  accepted offers are left to bill. A cancelled invoice and its Storno bill nothing.
- **Vorgang page, card `Dokumente`**: its documents as a graph, read from what relates them:
  - two columns, offers and invoices, each a list of trees by date: a Nachtrag, Alternative or
    Ersatzangebot under its offer, a Storno and Ersatzrechnung under the invoice they correct,
    joined by branches;
  - lines cross over for what bills or shares an offer, an Abschlag's line to what deducts it bends
    out right of the invoices; what a correction repeats of what it corrects is not drawn;
  - cards: title with the check marks behind it, day and amount, the status tags wrapping (the card
    grows, the lines are drawn to where the cards ended up); a click opens the document;
  - hovering a card lights up its lines and what they join; documents of other Vorgänge (a
    Sammelrechnung's offers) are dashed and open their Vorgang; an ordered list on a narrow card;
  - first tried as lanes like a CI pipeline view (a column per step): too many lines, no sense of
    time.
- A document opens inside its Vorgang (`/cases/case-12/7`), `Schließen` goes back to it.

Why a graph and not a row of page thumbnails: invoices look alike at thumbnail size, and a row hides
what a Vorgang is about, which invoice billed which offer, what is still open, what was deducted or
cancelled. The page picture is to show on hover instead.

## Open

- The first page of a document on hovering its card.
- Hovering a card lights up only the lines drawn: a Storno or an Ersatzrechnung does not light up
  with the Abschlag whose deduction it repeats. To be looked at in use.
- A Folgeangebot is linked Vorgang to Vorgang only, so the graph does not draw it (the Vorgang's
  references name it above).

## Views and lists

- Document views without cards, to gain space.
- The grouping in the settings has grown out too.

## App settings, next to backup and restore

- **Appearance**: light/dark override, overall scaling or font size, app behaviour, the direction of
  the histories (latest first, as now, or oldest first).
- **Security**: lock the app after a while, unlock with Touch ID or Windows Hello.
- **Autostart**.
- **MCP server**, maybe.
