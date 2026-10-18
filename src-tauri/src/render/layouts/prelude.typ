// What the layouts share: page setup that an own layout (stationery, margins, font) changes.
// `d` is the data of `data.json`, `d.page` the part of it that belongs here.

// the font of an own layout in front of the one the layout is made for
#let font-of(d, own) = if d.page.font != none { (d.page.font, own) } else { own }

// the margins of an own layout (millimetres), else the ones the layout brings
#let margin-of(d, own) = if d.page.margin == none { own } else {
  (
    top: d.page.margin.top * 1mm,
    right: d.page.margin.right * 1mm,
    bottom: d.page.margin.bottom * 1mm,
    left: d.page.margin.left * 1mm,
  )
}

// lines where the content may go, with the distance in millimetres, drawn over the stationery
#let guides-of(m) = {
  let mark = 0.6pt + rgb("#e5484d")
  let note(body) = text(size: 6pt, fill: rgb("#e5484d"), body)
  let mm(length) = [#calc.round(length / 1mm, digits: 1) mm]
  place(top + left, dy: m.top, line(length: 100%, stroke: mark))
  place(bottom + left, dy: -m.bottom, line(length: 100%, stroke: mark))
  place(top + left, dx: m.left, line(angle: 90deg, length: 100%, stroke: mark))
  place(top + right, dx: -m.right, line(angle: 90deg, length: 100%, stroke: mark))
  place(top + left, dx: m.left + 2pt, dy: m.top - 8pt, note(mm(m.top)))
  place(bottom + left, dx: m.left + 2pt, dy: -m.bottom + 2pt, note(mm(m.bottom)))
  place(top + left, dx: 2pt, dy: m.top + 20pt, note(mm(m.left)))
  place(top + right, dx: -2pt, dy: m.top + 20pt, note(mm(m.right)))
}

// the stationery behind the pages: one file for all, or one for the first and one for the others,
// with the margin guides when asked for (`others` and `first` as the layout uses them)
#let background-of(d, others, first) = if d.page.background == none and not d.page.guides { none } else {
  context {
    let page-number = counter(page).get().first()
    if d.page.background != none {
      let b = d.page.background
      let file = if page-number > 1 and b.rest != none { b.rest } else { b.first }
      place(top + left, image(file, width: 100%, height: 100%))
    }
    if d.page.guides {
      guides-of(if page-number == 1 and first != none { first } else { others })
    }
  }
}

// the margins of the first page when they differ from the others (millimetres), else none
#let first-margin-of(d) = if d.page.first_margin == none { none } else {
  (
    top: d.page.first_margin.top * 1mm,
    right: d.page.first_margin.right * 1mm,
    bottom: d.page.first_margin.bottom * 1mm,
    left: d.page.first_margin.left * 1mm,
  )
}

// Typst has one margin for all pages that follow a page rule, so a different first page is laid out
// in two runs: what fits on the first page (measured with its margins) and, on pages of their own
// with the margins of the others, the rest. A unit is a block that is not split, or a splittable
// `(parts: (...), make: parts => content)` like the rows of a table, which breaks between parts.
#let two-runs(first, others, units, gap) = context {
  let gap = gap.to-absolute()
  let width = 210mm - first.left - first.right
  let room = 297mm - first.top - first.bottom
  let height(content) = measure(width: width, content).height
  let used = 0pt
  let now = ()
  let later = ()
  for unit in units {
    if later.len() > 0 {
      later.push(unit)
      continue
    }
    if type(unit) == dictionary {
      // the constant part (a table header) plus as many parts as fit
      let fixed = height((unit.make)(()))
      let fits = 0
      let total = used + fixed
      for part in unit.parts {
        let next = height((unit.make)((part,))) - fixed
        if (now.len() > 0 or fits > 0) and total + next > room { break }
        total += next
        fits += 1
      }
      if fits > 0 { now.push((unit.make)(unit.parts.slice(0, fits))) }
      used = total + gap
      if fits < unit.parts.len() {
        later.push((unit.make)(unit.parts.slice(fits)))
      }
    } else {
      let needed = height(unit) + gap
      // the first unit stays whatever its size, the page needs something to start with
      if now.len() > 0 and used + needed > room {
        later.push(unit)
      } else {
        now.push(unit)
        used += needed
      }
    }
  }
  now.join(v(gap))
  if later.len() > 0 {
    page(margin: others, later.join(v(gap)))
  }
}

// an own layout prints on stationery that has its own footer: only the page numbers remain,
// and only where there is more than one page
#let own-footer(d, size, color) = context {
  if counter(page).final().first() > 1 {
    set text(size: size, fill: color)
    align(right)[Seite #counter(page).display() von #counter(page).final().first()]
  }
}
