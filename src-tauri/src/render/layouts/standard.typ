// Invoice template. Data comes as `data.json` next to this file (see `InvoiceView` in render/view.rs),
// all numbers and dates are already formatted, so the template only lays them out.
#import "prelude.typ": *
#let d = json("data.json")
#let muted = luma(110)
#let rule = 0.5pt + luma(190)

#set document(
  title: d.title + " " + d.number,
  author: d.seller.name,
  date: datetime(year: d.issue.year, month: d.issue.month, day: d.issue.day),
)
#set text(font: font-of(d, "Libertinus Serif"), size: 10pt, lang: "de")
#set par(leading: 0.55em)
#let others = margin-of(d)
#let first = first-margin-of(d)
#set page(
  paper: "a4",
  margin: if first == none { others } else { first },
  background: background-of(d),
)

// the content in blocks that are laid out one after the other, a different first page needs them apart
#let head = {
  text(size: 15pt, weight: "bold")[#d.title #d.number]
  v(0.4cm)
  if d.letter.intro != none [#d.letter.intro]
  v(0.3cm)
  for note in d.intro [#note \ ]
}

#let positions = (
  parts: d.lines.map(l => (
    l.pos,
    [#l.name #if l.description != none [\ #text(size: 8.5pt, fill: muted)[#l.description]]
     #for a in l.adjustments [\ #text(size: 8.5pt, fill: muted)[#a]]],
    l.quantity,
    l.unit,
    [#l.unit_price #if l.vat_rate != "" [#text(size: 8pt, fill: muted)[(#l.vat_rate)]]],
    l.net,
  )),
  make: rows => table(
    columns: (auto, 1fr, auto, auto, auto, auto),
    align: (left, left, right, left, right, right),
    stroke: (x, y) => (bottom: if y == 0 { 0.8pt } else { rule }),
    inset: (x: 4pt, y: 5pt),
    table.header[*Pos.*][*Beschreibung*][*Menge*][*Einheit*][*Einzelpreis*][*Netto*],
    ..rows.flatten(),
  ),
)

#let totals = align(right)[
  #block(width: 9.5cm)[
    #set text(size: 9.5pt)
    #table(
      columns: (1fr, auto),
      align: (left, right),
      stroke: none,
      inset: (x: 2pt, y: 3pt),
      ..d.totals.map(t => if t.strong {
        (table.hline(stroke: 0.8pt), text(weight: "bold")[#t.label], text(weight: "bold")[#t.value])
      } else {
        ([#t.label], [#t.value])
      }).flatten(),
    )
  ]
]

#let notes = for note in d.vat_notes [#text(size: 8.5pt, fill: muted)[#note] \ ]

// A line of several items (` · ` between them, like the bank details) breaks between the items, never
// inside one and never right before a dot: each item keeps its dot, they are joined by a space.
// A line with a long item is left to break wherever it must.
#let payment-line(line) = {
  let items = line.split(" · ")
  if items.len() > 1 and items.all(item => item.len() < 70) {
    items.enumerate().map(((i, item)) => box(if i < items.len() - 1 { item + " ·" } else { item })).join[ ]
  } else {
    line
  }
}

#let payment = if d.payment.len() > 0 [
  *Zahlung* \
  #for line in d.payment [#payment-line(line) \ ]
]

#let units = (
  head,
  positions,
  totals,
  notes,
  payment,
  if d.letter.closing != none { d.letter.closing },
).filter(unit => unit != none)

#let gap = 1.2em

#if first == none {
  units.map(unit => if type(unit) == dictionary { (unit.make)(unit.parts) } else { unit }).join(v(gap))
} else {
  two-runs(first, others, units, gap)
}
