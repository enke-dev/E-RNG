# UI follow-ups – Plan

Goal: more room and calmer views once the groundwork of [vorgaenge.md](vorgaenge.md) is in, plus app
settings that belong to the app, not to a context.

Status: being sharpened (2026-11-03), nothing built. The Vorgänge plan is done.

## Vorgänge: the list holds Vorgänge, the Vorgang shows its documents

- **Sidebar list**: only Vorgänge, grouped by customer (the group heading is the customer, so the
  items do not repeat it). An item: number, internal title, stage tag, the open or invoiced sum. The
  filters become Vorgang filters (stage, year, customer, context); the kind filter goes.
- **Vorgang page, card `Dokumente`**: its documents as a graph, read from what relates them, left
  to right by date:
  - nodes are small cards: kind and number (or `Entwurf`), amount, the status tags (`statusOf`); a
    click opens the document, hovering shows its first page;
  - edges say how they relate: an invoice billing an offer (solid), a share of an offer, an
    Anzahlung or Abschlag (dashed), a Schlussrechnung deducting them (lines joining), a Nachtrag,
    an alternative or a replacement (offer to offer), a Storno and its Ersatzrechnung (to the
    cancelled invoice);
  - documents of other Vorgänge (a Sammelrechnung's offers, a Folgeangebot) are ghost nodes at the
    edge, opening that Vorgang;
  - the layout is lanes like a CI pipeline view: a column per step (an offer, what bills it, what
    settles that), a lane per line of offers; scrolls sideways when long, an ordered list on a
    narrow window.
- A document opens inside its Vorgang (the Vorgang stays selected in the list, a way back to it).
- Then the "Angebote" and "Rechnungen" entries go, unless the flat lists earn their place (an
  overview of all open invoices, say).

Why a graph and not a row of page thumbnails ordered by date: invoices look alike at thumbnail size,
and a row hides what a Vorgang is about, which invoice billed which offer, what is still open, what
was deducted or cancelled. The relations are all in the data already (`relations`, `Position.source`
and `share`, `deductions`, `corrects`, `case_links`). The page picture shows on hover instead.

Open:

- The route of a document inside its Vorgang (`/cases/case-12/7`?), and what the editor's
  `Schließen` goes back to.
- A Vorgang with one document: the graph is one node; fine, or straight to the document?

## Views and lists

- Document views without cards, to gain space.
- Sidebar lists improved; the grouping in the settings has grown out too.

## App settings, next to backup and restore

- **Appearance**: light/dark override, overall scaling or font size, app behaviour, the direction of
  the histories (latest first, as now, or oldest first).
- **Security**: lock the app after a while, unlock with Touch ID or Windows Hello.
- **Autostart**.
- **MCP server**, maybe.
