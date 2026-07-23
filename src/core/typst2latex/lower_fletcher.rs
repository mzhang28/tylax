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
    side_right: bool,
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
        side_right,
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
    let mut nodes: Vec<(f64, f64, Content)> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    collect_coordinate(body, &mut nodes, &mut edges);

    // Build sorted, de-duplicated column (x) and row (y) axes.
    let mut xs: Vec<f64> = nodes.iter().map(|n| n.0).collect();
    let mut ys: Vec<f64> = nodes.iter().map(|n| n.1).collect();
    for e in &edges {
        for c in [e.from, e.to].into_iter().flatten() {
            xs.push(c.0);
            ys.push(c.1);
        }
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
    for edge in &edges {
        // Only edges between two distinct grid points map cleanly to a
        // tikz-cd arrow. Skip self-loops and coordinate-less edges (e.g. the
        // direction-only or bent edges in automaton-style diagrams) rather than
        // emitting an arrow to a non-existent cell, which breaks compilation.
        let (Some(from), Some(to)) = (edge.from, edge.to) else { continue };
        if (from.0 - to.0).abs() < 1e-6 && (from.1 - to.1).abs() < 1e-6 {
            continue;
        }
        // Direction is in terms of *grid indices*, not raw coordinates:
        // coordinates may be non-contiguous (e.g. columns 0 and 2 with nothing
        // at 1), and tikz-cd steps by cell, so a raw "rr" would overshoot.
        let dcol = col_of(to.0) as i64 - col_of(from.0) as i64;
        let drow = row_of(to.1) as i64 - row_of(from.1) as i64;
        let dir = index_dir(dcol, drow);
        let cell = &mut grid[row_of(from.1)][col_of(from.0)];
        cell.push(' ');
        cell.push_str(&arrow(&dir, edge, styles, ctx));
    }

    grid.iter()
        .map(|r| r.join(" & "))
        .collect::<Vec<_>>()
        .join(" \\\\\n")
}

fn collect_coordinate(content: &Content, nodes: &mut Vec<(f64, f64, Content)>, edges: &mut Vec<Edge>) {
    if let Some(dict) = marker(content, "fletcher-node") {
        if let (Some((x, y)), Some(b)) = (dict_coord(dict, "coord"), dict_content(dict, "body")) {
            nodes.push((x, y, b));
        }
        return;
    }
    if let Some(dict) = marker(content, "fletcher-edge") {
        edges.push(parse_edge(dict));
        return;
    }
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        for c in seq.children.iter() {
            collect_coordinate(c, nodes, edges);
        }
    } else if let Some(styled) = content.to_packed::<typst::foundations::StyledElem>() {
        collect_coordinate(&styled.child, nodes, edges);
    }
}

fn dedup_axis(v: &mut Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
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
