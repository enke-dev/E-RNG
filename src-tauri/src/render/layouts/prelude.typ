// What the layouts share: the frame of the content, the parts placed around it and the
// aids for making a layout. `d` is the data of `data.json`, `d.page` the part of it that
// belongs here (see render/parts.rs).

#let red = rgb("#e5484d")
#let cyan = rgb("#12a5c0")
#let orange = rgb("#f5a524")

// the font of an own layout in front of the one the layout is made for
#let font-of(d, own) = if d.page.font != none { (d.page.font, own) } else { own }

// insets in millimetres as lengths
#let insets-of(m) = (top: m.top * 1mm, right: m.right * 1mm, bottom: m.bottom * 1mm, left: m.left * 1mm)

// the frame of the pages after the first
#let margin-of(d) = insets-of(d.page.margin)

// the frame of the first page when it differs from the others, else none
#let first-margin-of(d) = if d.page.first_margin == none { none } else { insets-of(d.page.first_margin) }

// where a part is attached to the page, and which way its offsets count
#let anchors = (
  "top-left": top + left,
  "top-center": top + center,
  "top-right": top + right,
  "middle-left": horizon + left,
  "center": horizon + center,
  "middle-right": horizon + right,
  "bottom-left": bottom + left,
  "bottom-center": bottom + center,
  "bottom-right": bottom + right,
)

// a mark for an image that has too few pixels for its size, in the sample only
#let blurry-mark() = box(width: 5mm, height: 5mm, {
  place(polygon(fill: rgb("#f5a524"), stroke: 0.5pt + white, (2.5mm, 0mm), (5mm, 4.5mm), (0mm, 4.5mm)))
  place(center + bottom, dy: -0.2mm, text(size: 6pt, weight: "bold", fill: white)[!])
})

// the color a part was given (`#rrggbb`), else the one its kind has
#let color-of(part, default) = if part.color == none { default } else { rgb(part.color) }

#let part-body(d, part, base) = {
  let size = part.at("size", default: 9) * 1pt
  let family(own) = if own == none { base } else { (own, ..base) }
  let aligned = part.at("align", default: "left")
  let alignment = if aligned == "right" { right } else if aligned == "center" { center } else { left }
  if part.kind == "text" {
    set text(size: size, font: family(part.family))
    set text(fill: rgb(part.color)) if part.color != none
    set align(alignment)
    part.lines.map(runs => runs.map(r => text(
      weight: if r.bold { "bold" } else { "regular" },
      style: if r.italic { "italic" } else { "normal" },
      r.text,
    )).join()).join(linebreak())
  } else if part.kind == "infos" {
    set text(size: size)
    set text(fill: rgb(part.color)) if part.color != none
    if part.labels {
      // the rows hug the side the part is aligned to, the labels end where the values start
      align(alignment, grid(
        columns: (auto, auto),
        column-gutter: 10pt,
        row-gutter: 5pt,
        align: (right, left),
        ..part.rows.map(r => (text(fill: luma(110), r.label), r.value)).flatten(),
      ))
    } else {
      set align(alignment)
      part.rows.map(r => r.value).join(linebreak())
    }
  } else if part.kind == "address" {
    if part.sender != none {
      text(size: size * 0.7, fill: luma(110), part.sender)
      v(2pt)
    }
    set text(size: size)
    set text(fill: rgb(part.color)) if part.color != none
    strong(part.lines.first())
    for line in part.lines.slice(1) { linebreak(); line }
  } else if part.kind == "image" {
    image(part.file, width: 100%)
    if d.page.marks and part.low_res {
      place(top + right, dx: -1mm, dy: 1mm, blurry-mark())
    }
  } else if part.kind == "page-number" {
    set text(size: size, fill: color-of(part, luma(110)))
    set align(alignment)
    context part.text
      .replace("{page}", str(counter(page).get().first()))
      .replace("{pages}", str(counter(page).final().first()))
  } else if part.kind == "line" {
    line(length: 100%, stroke: part.thickness * 1pt + color-of(part, luma(190)))
  }
}

// one part, attached to its anchor, the offsets count from there towards the inside of the page;
// without a width the part is as wide as its content. In the sample the box of every part is
// handed out (`<part-box>`, millimetres from the top left of the page), so the app can tell what
// is under the pointer.
#let place-part(d, part, base, number, own) = {
  let anchor = part.anchor
  let dx = part.x * 1mm * if anchor.ends-with("right") { -1 } else { 1 }
  let dy = part.y * 1mm * if anchor.starts-with("bottom") { -1 } else { 1 }
  let outline = if d.page.guides { (paint: cyan, thickness: 0.8pt, dash: "dashed") } else { none }
  let fill = if d.page.guides { cyan.transparentize(90%) } else { none }
  let body = part-body(d, part, base)
  let element = if part.width == none {
    box(stroke: outline, fill: fill, body)
  } else {
    block(width: part.width * 1mm, stroke: outline, fill: fill, body)
  }
  place(anchors.at(anchor), dx: dx, dy: dy, element)
  if d.page.marks {
    let size = measure(element)
    let left = if anchor.ends-with("left") { dx } else if anchor.ends-with("right") { 210mm + dx - size.width } else { 105mm + dx - size.width / 2 }
    let above = if anchor.starts-with("top") { dy } else if anchor.starts-with("bottom") { 297mm + dy - size.height } else { 148.5mm + dy - size.height / 2 }
    [#metadata((
      page: number,
      first: own,
      index: part.index,
      x: left / 1mm,
      y: above / 1mm,
      w: size.width / 1mm,
      h: size.height / 1mm,
    )) <part-box>]
  }
}

// lines where the content may go, each with its distance in millimetres next to it on the inside
// of the frame: the vertical ones at the top, right of the left line and left of the right line, the
// horizontal ones in the strip at the left edge, under the top line and above the bottom one
#let guides-of(m) = {
  let mark = 0.6pt + red
  let note(body) = text(size: 6pt, fill: red, body)
  let mm(length) = [#calc.round(length / 1mm, digits: 1) mm]
  place(top + left, dy: m.top, line(length: 100%, stroke: mark))
  place(bottom + left, dy: -m.bottom, line(length: 100%, stroke: mark))
  place(top + left, dx: m.left, line(angle: 90deg, length: 100%, stroke: mark))
  place(top + right, dx: -m.right, line(angle: 90deg, length: 100%, stroke: mark))
  place(top + left, dx: 2pt, dy: m.top + 2pt, note(mm(m.top)))
  place(bottom + left, dx: 2pt, dy: -m.bottom - 2pt, note(mm(m.bottom)))
  place(top + left, dx: m.left + 2pt, dy: 2pt, note(mm(m.left)))
  place(top + right, dx: -m.right - 2pt, dy: 2pt, note(mm(m.right)))
}

// what a printer can print on: desktop printers leave about 5 mm at every edge blank
#let printable-area() = {
  let area = (paint: red, thickness: 0.6pt, dash: "dashed")
  place(top + left, dx: 5mm, dy: 5mm, rect(width: 100% - 10mm, height: 100% - 10mm, stroke: area))
}

// what helps to lay out a letter (DIN 5008 form B): the window of the address, the folds, the punch
// mark, the middle of the page; the holes of a two-hole punch are on every page
#let letter-aids(first) = {
  let aid = (paint: cyan, thickness: 0.5pt, dash: "dashed")
  let note(body) = text(size: 6pt, fill: cyan, body)
  place(top + left, dx: 105mm, line(angle: 90deg, length: 100%, stroke: aid))
  // a two-hole punch (ISO 838): 6 mm holes, 80 mm apart around the middle, their centres 12 mm from the edge
  for y in (108.5mm, 188.5mm) {
    place(top + left, dx: 12mm - 3mm, dy: y - 3mm, circle(radius: 3mm, stroke: 0.5pt + cyan))
  }
  // the four-hole pattern adds two more, 80 mm further out: dashed, as not every binder needs them
  for y in (28.5mm, 268.5mm) {
    place(top + left, dx: 12mm - 3mm, dy: y - 3mm, circle(radius: 3mm, stroke: aid))
  }
  if first {
    place(top + left, dx: 20mm, dy: 45mm, rect(width: 85mm, height: 45mm, stroke: aid))
    place(top + left, dx: 21mm, dy: 46mm, note[Anschriftfeld (DIN 5008 B)])
    for y in (105mm, 210mm) {
      place(top + left, dy: y, line(length: 100%, stroke: aid))
    }
    place(top + left, dy: 148.5mm, line(length: 6mm, stroke: 0.8pt + cyan))
  }
}

// everything behind the content of a page: the parts, the aids
#let background-of(d) = context {
  let number = counter(page).get().first()
  let first = number == 1
  let parts = if first and d.page.first_parts != none { d.page.first_parts } else { d.page.parts }
  let margin = if first and d.page.first_margin != none { first-margin-of(d) } else { margin-of(d) }
  let base = font-of(d, "Libertinus Serif")
  // the setup the page is laid out with: the first page's own, or the one of the others
  let own = first and d.page.first_parts != none
  for part in parts { place-part(d, part, base, number, own) }
  if d.page.guides {
    letter-aids(first)
    guides-of(margin)
    printable-area()
  }
}

// Typst has one margin for all pages that follow a page rule, so a different first page is laid out
// in two runs: what fits on the first page (measured with its margins) and, on pages of their own
// with the margins of the others, the rest. A unit is a block that is not split, or a splittable
// `(parts: (...), make: parts => content)` like the rows of a table, which breaks between parts.
#let two-runs(first, others, units, gap) = context {
  let gap = gap.to-absolute()
  let width = 210mm - first.left - first.right
  // a little less than the page has: rows measured on their own add up to a bit less than they take
  // in one table, and a unit that does not fit must not spill onto a page with the wrong frame
  let room = 297mm - first.top - first.bottom - 3mm
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
        let next = height((unit.make)((part,))) - fixed + 0.5pt
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
