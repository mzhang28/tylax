// Tylax compatibility shim for @preview/fletcher.
//
// Replaces fletcher's entrypoint. Instead of laying diagrams out via cetz, the
// `diagram`/`node`/`edge` constructors emit semantic `metadata` markers that
// Tylax lowers to LaTeX `tikz-cd`. Only the surface API the documents use is
// provided (diagram, node, edge); everything else fletcher exports is omitted.
//
// NOTE: this is the pragmatic per-package approach. The more general path would
// be to shim cetz (fletcher's rendering backend) directly.

// True for a fletcher direction string like "r", "rr", "dr", "ul", ...
#let _is-dir(s) = type(s) == str and s.match(regex("^[udlr]+$")) != none

// A diagram. The positional args are the diagram objects: either a single body
// (a math matrix, or a block of node()/edge() calls) or a variadic list of
// node()/edge() markers. Concatenate all content positionals into one body so
// lowering can walk it. Non-content positionals (spacing, etc.) are dropped —
// they only affect visual layout.
#let diagram(..args) = {
  let objs = args.pos().filter(a => type(a) == content)
  let body = if objs.len() == 0 { [] } else { objs.join() }
  [#metadata((type: "fletcher-diagram", body: body))]
}

// A node at an explicit grid coordinate (coordinate style).
#let node(..args) = {
  let pos = args.pos()
  let coord = if pos.len() > 0 { pos.at(0) } else { none }
  let body = if pos.len() > 1 { pos.at(1) } else { [] }
  [#metadata((type: "fletcher-node", coord: coord, body: body))]
}

// An edge. fletcher's positional args are heterogeneous; classify them:
//   - a direction string ("r"/"d"/"rr"/... )        -> dir
//   - any other string ("->", "<-", "=", ...)       -> marks (arrow style)
//   - an array of two coordinates                    -> from/to (coordinate style)
//   - any other array (e.g. (id, dot))               -> label (joined tuple)
//   - content                                        -> label
// The `label-side` named arg (left/right) is preserved.
#let edge(..args) = {
  let pos = args.pos()
  let named = args.named()
  let dir = none
  let marks = none
  let label = none
  let from = none
  let to = none
  let coords = ()
  for a in pos {
    if _is-dir(a) {
      dir = a
    } else if type(a) == str {
      marks = a
    } else if type(a) == array {
      // A coordinate looks like a 2-tuple of numbers/lengths; otherwise treat
      // the array as a tuple label such as (id, dot).
      let is-coord = a.len() == 2 and a.all(x => type(x) in (int, float, length))
      if is-coord {
        coords.push(a)
      } else if label == none {
        label = [(#a.map(x => [#x]).join([, ]))]
      }
    } else if type(a) == content {
      label = a
    }
  }
  if coords.len() >= 2 {
    from = coords.at(0)
    to = coords.at(1)
  } else if coords.len() == 1 and label == none {
    label = [(#coords.at(0).map(x => [#x]).join([, ]))]
  }
  [#metadata((
    type: "fletcher-edge",
    dir: dir,
    marks: marks,
    label: label,
    from: from,
    to: to,
    side: named.at("label-side", default: none),
  ))]
}
