# UI follow-ups – Plan

Goal: more room and calmer views once the groundwork of [vorgaenge.md](vorgaenge.md) is in, plus app
settings that belong to the app, not to a context.

Status (2026-11-03): the Vorgänge list by customer and the document graph are built; the rest is idea.

## Done: the list holds Vorgänge, the Vorgang shows its documents

- **Sidebar list**: only Vorgänge, grouped by customer (the group heading is the customer, so the
  items do not repeat it). An item: number, internal title, stage tag, the open or invoiced sum. The
  filters become Vorgang filters (stage, year, customer, context); the kind filter goes.
- **Vorgang page, card `Dokumente`**: its documents as a graph, read from what relates them:
  - nodes are small cards: kind and number (or `Entwurf`), amount, the status tags (`statusOf`); a
    click opens the document (its first page on hovering: not built yet);
  - edges say how they relate: an invoice billing an offer (solid), a share of an offer, an
    Anzahlung or Abschlag (dashed), a Schlussrechnung deducting them (lines joining), a Nachtrag,
    an alternative or a replacement (offer to offer), a Storno and its Ersatzrechnung (to the
    cancelled invoice);
  - documents of other Vorgänge (a Sammelrechnung's offers, a Folgeangebot) are ghost nodes at the
    edge, opening that Vorgang;
  - the layout is two columns, offers and invoices, each a list of trees by date: a Nachtrag,
    Alternative or Ersatzangebot under its offer, a Storno and Ersatzrechnung under the invoice
    they correct, joined by branches; lines cross over for what bills or shares an offer, an
    Abschlag's line to what deducts it bends out right of the invoices; what a correction repeats of
    what it corrects (the same offers billed, the same Abschlag deducted) is not drawn; hovering a
    card lights up its lines and what they join; an ordered list on a narrow window.
  - first tried as lanes like a CI pipeline view (a column per step, lines skipping steps above the
    row): too many lines, no sense of time.
- A document opens inside its Vorgang (`/cases/case-12/7`, the Vorgang stays selected in the list,
  `Schließen` goes back to it).
- A Vorgang with one document shows a graph of one card.
- The "Angebote" and "Rechnungen" entries stay for now, to get a feel for the new list; they may go
  later, unless the flat lists earn their place (an overview of all open invoices, say).

Why a graph and not a row of page thumbnails ordered by date: invoices look alike at thumbnail size,
and a row hides what a Vorgang is about, which invoice billed which offer, what is still open, what
was deducted or cancelled. The relations are all in the data already (`relations`, `Position.source`
and `share`, `deductions`, `corrects`, `case_links`). The page picture shows on hover instead.

Open:

- The first page of a document on hovering its card.
- A Folgeangebot is linked Vorgang to Vorgang only, so the graph does not draw it (the Vorgang's
  references name it above).

## Views and lists

- Document views without cards, to gain space.
- Sidebar lists improved; the grouping in the settings has grown out too.

## App settings, next to backup and restore

- **Appearance**: light/dark override, overall scaling or font size, app behaviour, the direction of
  the histories (latest first, as now, or oldest first).
- **Security**: lock the app after a while, unlock with Touch ID or Windows Hello.
- **Autostart**.
- **MCP server**, maybe.
