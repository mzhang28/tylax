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

// A node at an explicit grid coordinate (coordinate style). May carry a
// `name: <label>` so edges can reference it by name.
#let node(..args) = {
  let pos = args.pos()
  let named = args.named()
  let coord = if pos.len() > 0 { pos.at(0) } else { none }
  let body = if pos.len() > 1 { pos.at(1) } else { [] }
  let name = named.at("name", default: none)
  [#metadata((
    type: "fletcher-node",
    coord: coord,
    body: body,
    name: if name != none { str(name) } else { none },
  ))]
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
  // NB: do not name this `label` — that would shadow the built-in `label`
  // type and break the `type(a) == label` check below.
  let lbl = none
  let from = none
  let to = none
  let coords = ()
  let refs = ()   // node-name references (`<name>`) used as endpoints
  for a in pos {
    if _is-dir(a) {
      dir = a
    } else if type(a) == str {
      marks = a
    } else if type(a) == label {
      refs.push(str(a))
    } else if type(a) == array {
      // A coordinate looks like a 2-tuple of numbers/lengths; otherwise treat
      // the array as a tuple label such as (id, dot).
      let is-coord = a.len() == 2 and a.all(x => type(x) in (int, float, length))
      if is-coord {
        coords.push(a)
      } else if lbl == none {
        lbl = [(#a.map(x => [#x]).join([, ]))]
      }
    } else {
      // Anything else (content, or a bare `symbol` like `alpha`/`eta` — which
      // in math is NOT `content`) is the edge label. Coerce to content.
      lbl = [#a]
    }
  }
  // A numeric 2-tuple is always a coordinate: 1 → `from`, 2 → `from`/`to`.
  // (Non-coordinate tuples like `(id, dot)` were classified as labels above.)
  if coords.len() >= 2 {
    from = coords.at(0)
    to = coords.at(1)
  } else if coords.len() == 1 {
    from = coords.at(0)
  }
  [#metadata((
    type: "fletcher-edge",
    dir: dir,
    marks: marks,
    label: lbl,
    from: from,
    to: to,
    from-name: if refs.len() >= 1 { refs.at(0) } else { none },
    to-name: if refs.len() >= 2 { refs.at(1) } else { none },
    side: named.at("label-side", default: none),
  ))]
}
