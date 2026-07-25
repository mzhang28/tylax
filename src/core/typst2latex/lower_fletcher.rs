//! Lower fletcher diagrams (captured as semantic markers by the bundled
//! `packages/fletcher.typ` shim) to LaTeX `tikz-cd`.
//!
//! Two authoring styles are handled:
//!   * matrix style — `diagram($ A edge("r","->") & B \ C & D $)`: a math
//!     matrix whose cells carry `edge(...)` markers. Maps almost 1:1 to tikz-cd
//!     (fletcher's `"r"`/`"d"`/`"rr"` direction strings are tikz-cd directions).
//!   * coordinate style — `diagram({ node((x,y), …) … edge((x0,y0),(x1,y1),…) })`:
//!     explicit grid coordinates; we build a matrix and route arrows by the
//!     coordinate delta.

use typst::foundations::{Content, SequenceElem, StyleChain, Value, Str, Dict};
use typst::introspection::MetadataElem;
use typst::math::{AlignPointElem};
use typst::text::LinebreakElem;

use crate::core::typst2latex::ir::LatexIr;
use crate::core::typst2latex::lower::LowerContext;
use crate::core::typst2latex::lower_math::lower_math_fragment;

struct Edge {
    dir: Option<String>,
    marks: Option<String>,
    label: Option<Content>,
    from: Option<(f64, f64)>,
    to: Option<(f64, f64)>,
    from_name: Option<String>,
    to_name: Option<String>,
    side_right: bool,
    shift: i64,
    bend: f64,
    invisible: bool,
}

/// Entry point: lower a `fletcher-diagram` marker body to a `tikz-cd`.
pub fn lower_diagram(body: &Content, styles: StyleChain, ctx: &mut LowerContext) -> LatexIr {
    // Center a standalone diagram, but emit bare inside a table cell (display
    // math / `center` break inside a `tabular`).
    let centered = !ctx.in_table_cell;
    // Coordinate style iff the body contains any `fletcher-node` marker.
    let rows = if contains_node_marker(body) {
        coordinate_rows(body, styles, ctx)
    } else {
        matrix_rows(body, styles, ctx)
    };
    tikzcd(rows, centered)
}

fn tikzcd(rows: String, centered: bool) -> LatexIr {
    let env = format!("\\begin{{tikzcd}}\n{rows}\n\\end{{tikzcd}}");
    if centered {
        // A standalone diagram is centered on its own line.
        LatexIr::Latex(format!("\n\\begin{{center}}\n{env}\n\\end{{center}}\n"))
    } else {
        // Inside a table cell: bare (no `center`/display-math wrapper).
        LatexIr::Latex(format!("\n{env}\n"))
    }
}

// ---------------------------------------------------------------------------
// Marker reading helpers
// ---------------------------------------------------------------------------

fn marker<'a>(content: &'a Content, kind: &str) -> Option<&'a Dict> {
    let md = content.to_packed::<MetadataElem>()?;
    let Value::Dict(dict) = &md.value else { return None };
    match dict.get(&Str::from("type")).ok()? {
        Value::Str(t) if t.as_str() == kind => Some(dict),
        _ => None,
    }
}

fn dict_str(dict: &Dict, key: &str) -> Option<String> {
    match dict.get(&Str::from(key)).ok()? {
        Value::Str(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn dict_content(dict: &Dict, key: &str) -> Option<Content> {
    match dict.get(&Str::from(key)).ok()? {
        Value::Content(c) => Some(c.clone()),
        _ => None,
    }
}

fn dict_coord(dict: &Dict, key: &str) -> Option<(f64, f64)> {
    let Value::Array(a) = dict.get(&Str::from(key)).ok()? else { return None };
    if a.len() != 2 {
        return None;
    }
    let num = |v: Value| match v {
        Value::Int(i) => Some(i as f64),
        Value::Float(f) => Some(f),
        _ => None,
    };
    Some((num(a.at(0, None).ok()?)?, num(a.at(1, None).ok()?)?))
}

fn parse_edge(dict: &Dict) -> Edge {
    let side_right = matches!(dict.get(&Str::from("side")).ok(), Some(v) if format!("{v:?}").contains("Right"));
    Edge {
        dir: dict_str(dict, "dir"),
        marks: dict_str(dict, "marks"),
        label: dict_content(dict, "label"),
        from: dict_coord(dict, "from"),
        to: dict_coord(dict, "to"),
        from_name: dict_str(dict, "from-name"),
        to_name: dict_str(dict, "to-name"),
        side_right,
        shift: match dict.get(&Str::from("shift")).ok() {
            Some(Value::Int(i)) => *i,
            _ => 0,
        },
        bend: match dict.get(&Str::from("bend")).ok() {
            Some(Value::Float(f)) => *f,
            Some(Value::Int(i)) => *i as f64,
            _ => 0.0,
        },
        invisible: matches!(dict.get(&Str::from("invisible")).ok(), Some(Value::Bool(true))),
    }
}

/// Find a `fletcher-edge` marker dict anywhere within `content` (edges embedded
/// in a math matrix get wrapped in nested equations/sequences, so a top-level
/// check is not enough). Returns the first one found.
fn find_edge_marker(content: &Content) -> Option<&Dict> {
    if let Some(dict) = marker(content, "fletcher-edge") {
        return Some(dict);
    }
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        return seq.children.iter().find_map(find_edge_marker);
    }
    if let Some(styled) = content.to_packed::<typst::foundations::StyledElem>() {
        return find_edge_marker(&styled.child);
    }
    if let Some(eq) = content.to_packed::<typst::math::EquationElem>() {
        return find_edge_marker(&eq.body);
    }
    None
}

fn contains_node_marker(content: &Content) -> bool {
    if marker(content, "fletcher-node").is_some() {
        return true;
    }
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        return seq.children.iter().any(contains_node_marker);
    }
    if let Some(eq) = content.to_packed::<typst::math::EquationElem>() {
        return contains_node_marker(&eq.body);
    }
    false
}

// ---------------------------------------------------------------------------
// Arrow rendering
// ---------------------------------------------------------------------------

/// Map a fletcher arrow-mark string to a tikz-cd arrow-style option.
fn mark_option(marks: &Option<String>) -> Option<&'static str> {
    match marks.as_deref() {
        None | Some("->") | Some("-->") => None, // default arrow
        Some("<-") => Some("leftarrow"),
        Some("<->") => Some("leftrightarrow"),
        Some("=") | Some("==") => Some("equal"),
        Some("|->") => Some("mapsto"),
        Some(">->") | Some("hook->") => Some("hook"),
        Some("->>") => Some("twoheadrightarrow"),
        _ => None,
    }
}

/// Build a `\arrow[...]` command for an edge with an explicit direction.
fn arrow(dir: &str, edge: &Edge, styles: StyleChain, ctx: &mut LowerContext) -> String {
    let mut opts: Vec<String> = vec![dir.to_string()];
    if let Some(m) = mark_option(&edge.marks) {
        opts.push(m.to_string());
    }
    // `bend`/`shift` separate parallel arrows between the same node pair; an
    // invisible (`stroke: 0pt`) edge just carries a label.
    if edge.invisible {
        opts.push("draw=none".to_string());
    }
    if edge.bend.abs() > 0.5 {
        let side = if edge.bend > 0.0 { "left" } else { "right" };
        opts.push(format!("bend {}={}", side, edge.bend.abs().round() as i64));
    } else if edge.shift != 0 {
        // shift and bend don't combine well; prefer bend when both are present.
        opts.push(if edge.shift < 0 { "shift left".to_string() } else { "shift right".to_string() });
    }
    if let Some(label) = &edge.label {
        let text = lower_math_fragment(label, styles, ctx);
        if !text.trim().is_empty() {
            let swap = if edge.side_right { "'" } else { "" };
            // Brace the label: tikz's key parser splits on commas, so a comma
            // inside the (math) label would otherwise break the arrow options.
            opts.push(format!("\"{{{}}}\"{}", text.trim(), swap));
        }
    }
    format!("\\arrow[{}]", opts.join(", "))
}

// ---------------------------------------------------------------------------
// Matrix style
// ---------------------------------------------------------------------------

fn matrix_rows(body: &Content, styles: StyleChain, ctx: &mut LowerContext) -> String {
    // Unwrap the equation to reach the matrix sequence.
    let inner = if let Some(eq) = body.to_packed::<typst::math::EquationElem>() {
        &eq.body
    } else {
        body
    };
    let children: Vec<&Content> = if let Some(seq) = inner.to_packed::<SequenceElem>() {
        seq.children.iter().collect()
    } else {
        vec![inner]
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell_nodes: Vec<Content> = Vec::new();
    let mut cell_edges: Vec<Edge> = Vec::new();

    // Renders the accumulated cell (node math + its arrows) and resets it.
    macro_rules! flush_cell {
        () => {{
            let math = if cell_nodes.is_empty() {
                String::new()
            } else {
                lower_math_fragment(&Content::sequence(cell_nodes.drain(..)), styles, ctx)
            };
            cell_nodes.clear();
            // Empty cells still need an (invisible) node so that arrows
            // originating here or landing here have a tikz-cd shape to attach
            // to; a truly empty cell has no shape.
            let mut cell = if math.trim().is_empty() { "{}".to_string() } else { math.trim().to_string() };
            for edge in cell_edges.drain(..) {
                let dir = edge.dir.clone().unwrap_or_else(|| "r".to_string());
                cell.push(' ');
                cell.push_str(&arrow(&dir, &edge, styles, ctx));
            }
            row.push(cell);
        }};
    }

    for child in children {
        if child.is::<AlignPointElem>() {
            flush_cell!();
        } else if child.is::<LinebreakElem>() {
            flush_cell!();
            rows.push(std::mem::take(&mut row));
        } else if let Some(dict) = find_edge_marker(child) {
            cell_edges.push(parse_edge(dict));
        } else {
            cell_nodes.push(child.clone());
        }
    }
    flush_cell!();
    rows.push(row);

    // Pad ragged rows to a uniform column count so that directional arrows
    // (`rr`, `d`, ...) always have a target cell to land on.
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for r in &mut rows {
        while r.len() < width {
            r.push("{}".to_string());
        }
    }

    rows.iter()
        .map(|r| r.join(" & "))
        .collect::<Vec<_>>()
        .join(" \\\\\n")
}

// ---------------------------------------------------------------------------
// Coordinate style
// ---------------------------------------------------------------------------

fn coordinate_rows(body: &Content, styles: StyleChain, ctx: &mut LowerContext) -> String {
    // Collect nodes and edges *in document order* so that fletcher's implicit
    // edges (an `edge(...)` with no coordinates, which connects the preceding
    // node to the following node in argument order) can be resolved.
    let mut items: Vec<Item> = Vec::new();
    let mut cursor = (0.0, 0.0);
    collect_items(body, &mut items, &mut cursor);

    let nodes: Vec<(f64, f64, Content)> = items
        .iter()
        .filter_map(|it| match it {
            Item::Node(x, y, c, _) => Some((*x, *y, c.clone())),
            _ => None,
        })
        .collect();

    // Map node names (`node(.., name: <foo>)`) to coordinates, so edges can
    // reference endpoints by name.
    let by_name: std::collections::HashMap<&str, (f64, f64)> = items
        .iter()
        .filter_map(|it| match it {
            Item::Node(x, y, _, Some(name)) => Some((name.as_str(), (*x, *y))),
            _ => None,
        })
        .collect();

    // Resolve each edge to a concrete (from, to) coordinate pair:
    //   * explicit `from`/`to` coordinates, or `from-name`/`to-name` references;
    //   * `from` + direction string → step one grid cell per letter;
    //   * no coordinates → connect the nearest node before to the nearest node
    //     after this edge in document order (fletcher's implicit connection).
    let mut resolved: Vec<((f64, f64), (f64, f64), &Edge)> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let Item::Edge(e) = it else { continue };
        let from = e.from.or_else(|| e.from_name.as_deref().and_then(|n| by_name.get(n).copied()));
        let to = e.to.or_else(|| e.to_name.as_deref().and_then(|n| by_name.get(n).copied()));
        let pair = if let Some(from) = from {
            let to = to.or_else(|| e.dir.as_deref().map(|d| {
                let (dc, dr) = dir_delta(d);
                (from.0 + dc as f64, from.1 + dr as f64)
            }));
            to.map(|to| (from, to))
        } else if to.is_none() && e.dir.is_none() {
            // Implicit: previous node → next node.
            let before = items[..i].iter().rev().find_map(node_coord);
            let after = items[i + 1..].iter().find_map(node_coord);
            before.zip(after)
        } else {
            None
        };
        if let Some((from, to)) = pair {
            resolved.push((from, to, e));
        }
    }

    // Build sorted, de-duplicated column (x) and row (y) axes.
    let mut xs: Vec<f64> = nodes.iter().map(|n| n.0).collect();
    let mut ys: Vec<f64> = nodes.iter().map(|n| n.1).collect();
    for (from, to, _) in &resolved {
        xs.push(from.0); xs.push(to.0);
        ys.push(from.1); ys.push(to.1);
    }
    dedup_axis(&mut xs);
    dedup_axis(&mut ys);
    let col_of = |x: f64| xs.iter().position(|v| (*v - x).abs() < 1e-6).unwrap_or(0);
    let row_of = |y: f64| ys.iter().position(|v| (*v - y).abs() < 1e-6).unwrap_or(0);

    // Grid of cell strings; arrows are appended to their originating cell.
    // Cells default to an invisible `{}` node so every position has a tikz-cd
    // shape for arrows to attach to.
    let mut grid: Vec<Vec<String>> = vec![vec!["{}".to_string(); xs.len()]; ys.len()];
    for (x, y, content) in &nodes {
        grid[row_of(*y)][col_of(*x)] = lower_math_fragment(content, styles, ctx).trim().to_string();
    }
    for (from, to, edge) in &resolved {
        let (from, to) = (*from, *to);
        let cell_rc = (row_of(from.1), col_of(from.0));
        if (from.0 - to.0).abs() < 1e-6 && (from.1 - to.1).abs() < 1e-6 {
            // Self-loop (e.g. an automaton state transitioning to itself).
            let a = arrow("loop above", edge, styles, ctx);
            let cell = &mut grid[cell_rc.0][cell_rc.1];
            cell.push(' ');
            cell.push_str(&a);
            continue;
        }
        // Direction is in terms of *grid indices*, not raw coordinates:
        // coordinates may be non-contiguous (e.g. columns 0 and 2 with nothing
        // at 1), and tikz-cd steps by cell, so a raw "rr" would overshoot.
        let dcol = col_of(to.0) as i64 - col_of(from.0) as i64;
        let drow = row_of(to.1) as i64 - row_of(from.1) as i64;
        let dir = index_dir(dcol, drow);
        let cell = &mut grid[cell_rc.0][cell_rc.1];
        cell.push(' ');
        cell.push_str(&arrow(&dir, edge, styles, ctx));
    }

    grid.iter()
        .map(|r| r.join(" & "))
        .collect::<Vec<_>>()
        .join(" \\\\\n")
}

/// A diagram object in document order.
enum Item {
    Node(f64, f64, Content, Option<String>),
    Edge(Edge),
}

fn node_coord(it: &Item) -> Option<(f64, f64)> {
    match it {
        Item::Node(x, y, _, _) => Some((*x, *y)),
        _ => None,
    }
}

fn collect_items(content: &Content, items: &mut Vec<Item>, cursor: &mut (f64, f64)) {
    if let Some(dict) = marker(content, "fletcher-node") {
        if let (Some(coord), Some(b)) = (node_abs_coord(dict, cursor), dict_content(dict, "body")) {
            items.push(Item::Node(coord.0, coord.1, b, dict_str(dict, "name")));
        }
        return;
    }
    if let Some(dict) = marker(content, "fletcher-edge") {
        items.push(Item::Edge(parse_edge(dict)));
        return;
    }
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        for c in seq.children.iter() {
            collect_items(c, items, cursor);
        }
    } else if let Some(styled) = content.to_packed::<typst::foundations::StyledElem>() {
        collect_items(&styled.child, items, cursor);
    }
}

/// Resolve a node's coordinate to an absolute `(x, y)`, updating the running
/// `cursor`. Handles both an absolute `[x, y]` and a relative `(rel: (dx, dy))`
/// coordinate (fletcher lets a node be positioned relative to the previous one).
fn node_abs_coord(dict: &Dict, cursor: &mut (f64, f64)) -> Option<(f64, f64)> {
    match dict.get(&Str::from("coord")).ok()? {
        Value::Array(_) => {
            let c = dict_coord(dict, "coord")?;
            *cursor = c;
            Some(c)
        }
        Value::Dict(d) => {
            // Relative: `(rel: (dx, dy))`.
            let Value::Array(a) = d.get(&Str::from("rel")).ok()? else { return None };
            if a.len() != 2 {
                return None;
            }
            let num = |v: Value| match v {
                Value::Int(i) => Some(i as f64),
                Value::Float(f) => Some(f),
                _ => None,
            };
            let dx = num(a.at(0, None).ok()?)?;
            let dy = num(a.at(1, None).ok()?)?;
            *cursor = (cursor.0 + dx, cursor.1 + dy);
            Some(*cursor)
        }
        _ => None,
    }
}

fn dedup_axis(v: &mut Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
}

/// Grid delta `(dcol, drow)` for a fletcher direction string like "r"/"dr"/"uu".
fn dir_delta(dir: &str) -> (i64, i64) {
    let mut dc = 0;
    let mut dr = 0;
    for ch in dir.chars() {
        match ch {
            'r' => dc += 1,
            'l' => dc -= 1,
            'd' => dr += 1,
            'u' => dr -= 1,
            _ => {}
        }
    }
    (dc, dr)
}

/// tikz-cd direction string from grid-index deltas (`dcol`/`drow`). fletcher's
/// +y is down, matching tikz-cd's `d`.
fn index_dir(dcol: i64, drow: i64) -> String {
    let mut s = String::new();
    for _ in 0..drow.max(0) { s.push('d'); }
    for _ in 0..(-drow).max(0) { s.push('u'); }
    for _ in 0..dcol.max(0) { s.push('r'); }
    for _ in 0..(-dcol).max(0) { s.push('l'); }
    if s.is_empty() { s.push('r'); }
    s
}
