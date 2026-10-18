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
#let others = margin-of(d, (top: 2cm, bottom: 3.6cm, left: 2.5cm, right: 2cm))
#let first = first-margin-of(d)
#set page(
  paper: "a4",
  margin: if first == none { others } else { first },
  background: background-of(d, others, first),
  footer: if d.page.margin != none { own-footer(d, 7.5pt, muted) } else { context {
    set text(size: 7.5pt, fill: muted)
    line(length: 100%, stroke: rule)
    v(2pt)
    grid(
      columns: (1fr, 1fr, 1fr),
      column-gutter: 8pt,
      [
        *#d.seller.name* \
        #d.seller.address.join(linebreak())
      ],
      [
        #if d.seller.contact.len() > 0 [#d.seller.contact.join(linebreak())]
      ],
      [
        #if d.seller.registration.len() > 0 [#d.seller.registration.join(linebreak())]
      ],
    )
    if d.footer != none [
      #v(2pt)
      #d.footer
    ]
    align(right)[Seite #counter(page).display() von #counter(page).final().first()]
  } },
)

// the content in blocks that are laid out one after the other, a different first page needs them apart
#let head = {
  // Header: logo left, sender line + recipient block like a DIN 5008 window letter
  grid(
    columns: (1fr, 1fr),
    align: (left + top, right + top),
    if d.logo != none { image(d.logo, height: 1.6cm) } else { text(size: 14pt, weight: "bold")[#d.seller.name] },
    [],
  )
  v(1.2cm)
  grid(
    columns: (8.5cm, 1fr),
    column-gutter: 1cm,
    [
      #text(size: 7pt, fill: muted)[#d.seller.name · #d.seller.address.join(" · ")]
      #v(2pt)
      *#d.buyer.name* \
      #d.buyer.address.join(linebreak())
    ],
    align(right)[
      #set text(size: 9pt)
      #grid(
        columns: (auto, auto),
        column-gutter: 10pt,
        row-gutter: 5pt,
        align: (right, left),
        ..d.meta.map(m => (text(fill: muted)[#m.label], [#m.value])).flatten(),
      )
    ],
  )
  v(1cm)
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
    [#l.unit_price #text(size: 8pt, fill: muted)[(#l.vat_rate)]],
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

#let payment = if d.payment.len() > 0 [
  *Zahlung* \
  #for line in d.payment [#line \ ]
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
