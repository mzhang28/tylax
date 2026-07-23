use typst::math::EquationElem;
use typst::foundations::{Content, SequenceElem, StyledElem, StyleChain};
use typst::text::{TextElem, SpaceElem, RawElem, SmartQuoteElem};
use typst::model::{HeadingElem, ListItem, EnumItem, TermItem, LinkElem, ParbreakElem, StrongElem, EmphElem, RefElem, FigureElem, TableElem, TableChild, TableItem, FootnoteElem, FootnoteBody, CiteElem, CiteGroup, BibliographyElem};
use typst::introspection::StateUpdateElem;
use typst::loading::DataSource;
use typst::foundations::PathOrStr;
use std::collections::HashSet;
use typst::layout::{BlockElem, BoxElem, HElem, StackElem, StackChild, AlignElem};
use typst::visualize::RectElem;
use typst::foundations::SymbolElem;
use typst::introspection::MetadataElem;
use typst::foundations::Value;
use typst::World;
use crate::core::typst2latex::ir::LatexIr;

pub struct LowerContext<'a> {
    pub world: &'a dyn World,
    pub engine: typst::engine::Engine<'a>,
    pub locator: typst::introspection::SplitLocator<'a>,
    pub arenas: &'a typst::routines::Arenas,
    pub styles: typst::foundations::StyleChain<'a>,
    /// Unsupported constructs encountered while lowering (element name +
    /// human-readable source location + originating package). `convert`
    /// enforces the configured [`UnsupportedMode`] against this list.
    pub unsupported: Vec<UnsupportedInfo>,
    /// All labels defined in the document (`<label>`), collected in a pre-pass.
    /// Used to tell a cross-reference (`@key` → a defined label → `\ref`) from
    /// a citation (`@key` → a bibliography entry → `\cite`).
    pub defined_labels: HashSet<String>,
    /// `.bib` resources referenced by `#bibliography(...)`, for `\addbibresource`.
    pub bib_resources: Vec<String>,
}

/// A Typst construct Tylax could not faithfully lower, with provenance.
#[derive(Debug, Clone)]
pub struct UnsupportedInfo {
    pub name: String,
    pub location: Option<String>,
    pub package: Option<String>,
}

impl<'a> LowerContext<'a> {
    /// Record an unsupported element, capturing its source location and the
    /// package it originated from (via the span's file id) where available.
    pub fn record_unsupported(&mut self, name: &str, span: typst::syntax::Span) -> LatexIr {
        let mut location = None;
        let mut package = None;
        if let Some(id) = span.id() {
            if let typst::syntax::VirtualRoot::Package(spec) = id.root() {
                package = Some(format!("@{}/{}:{}", spec.namespace, spec.name, spec.version));
            }
            let path = id.vpath().get_without_slash().to_string();
            if let Ok(src) = self.world.source(id) {
                if let Some(node) = src.find(span) {
                    let start = node.range().start;
                    if let Some((line, col)) = src.lines().byte_to_line_column(start) {
                        location = Some(format!("{path}:{}:{}", line + 1, col + 1));
                    } else {
                        location = Some(path);
                    }
                } else {
                    location = Some(path);
                }
            } else {
                location = Some(path);
            }
        }
        self.unsupported.push(UnsupportedInfo { name: name.to_string(), location, package });
        LatexIr::Unsupported(name.to_string())
    }
}

/// Lower an already-*realized* piece of evaluated Typst content into `LatexIr`.
///
/// `styles` must be the real `StyleChain` this `content` was realized/grounded
/// under (e.g. the per-pair `StyleChain` returned by `typst_realize::realize`,
/// or `styles.chain(&styled.styles)` when unwrapping a `StyledElem`). Reading
/// styles off `StyleChain::default()` instead would silently ignore every
/// `#set`/`#show`-driven style (heading numbering level, equation `block`
/// flag, raw-block `lang`, smart-quote style, ...).
///
/// This wrapper also emits a `\label{…}` after any element that carries a
/// Typst label (`<name>`), so cross-references resolve.
pub fn lower_content<'s>(content: &Content, styles: StyleChain<'s>, ctx: &mut LowerContext) -> LatexIr {
    let ir = lower_content_inner(content, styles, ctx);
    if let Some(label) = content.label() {
        let key = label.resolve().to_string();
        return LatexIr::Sequence(vec![ir, LatexIr::Latex(format!("\\label{{{key}}}"))]);
    }
    ir
}

fn lower_content_inner<'s>(content: &Content, styles: StyleChain<'s>, ctx: &mut LowerContext) -> LatexIr {
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        let mut children = Vec::new();
        let mut current_list: Vec<LatexIr> = Vec::new();
        let mut list_is_numbered = false;

        fn push_list(children: &mut Vec<LatexIr>, list: &mut Vec<LatexIr>, numbered: bool) {
            if !list.is_empty() {
                let items = std::mem::take(list);
                children.push(if numbered {
                    LatexIr::NumberedList(items)
                } else {
                    LatexIr::List(items)
                });
            }
        }

        for child in seq.children.iter() {
            let ir = lower_content(child, styles, ctx);
            let is_numbered_item = child.to_packed::<EnumItem>().is_some();

            if matches!(ir, LatexIr::Item(_)) {
                if !current_list.is_empty() && list_is_numbered != is_numbered_item {
                    // A bullet item followed a numbered item (or vice versa):
                    // flush the previous run before starting a new list kind.
                    push_list(&mut children, &mut current_list, list_is_numbered);
                }
                list_is_numbered = is_numbered_item;
                current_list.push(ir);
            } else if matches!(ir, LatexIr::Space) {
                if !current_list.is_empty() {
                    // Ignore space between list items
                } else {
                    children.push(ir);
                }
            } else if matches!(ir, LatexIr::Parbreak) {
                push_list(&mut children, &mut current_list, list_is_numbered);
                children.push(ir);
            } else {
                push_list(&mut children, &mut current_list, list_is_numbered);
                children.push(ir);
            }
        }
        push_list(&mut children, &mut current_list, list_is_numbered);

        return LatexIr::Sequence(children);
    }

    if let Some(styled) = content.to_packed::<StyledElem>() {
        let chained = styles.chain(&styled.styles);
        return lower_content(&styled.child, chained, ctx);
    }

    if let Some(eq) = content.to_packed::<EquationElem>() {
        return crate::core::typst2latex::lower_math::lower_equation(eq, styles, ctx);
    }

    if content.is::<SpaceElem>() {
        return LatexIr::Space;
    }

    if content.is::<ParbreakElem>() {
        return LatexIr::Parbreak;
    }

    if let Some(text) = content.to_packed::<TextElem>() {
        return LatexIr::Text(text.text.to_string());
    }

    if let Some(heading) = content.to_packed::<HeadingElem>() {
        // `level` is `Smart::Auto` until synthesized (which only happens during
        // full realization); resolve it the same way Typst does, from
        // `offset + depth`, so `=`/`==`/`===` map to the right sectioning level.
        let level = heading.resolve_level(styles).get();
        return LatexIr::Heading { level, content: Box::new(lower_content(&heading.body, styles, ctx)) };
    }

    if let Some(item) = content.to_packed::<ListItem>() {
        return LatexIr::Item(Box::new(lower_content(&item.body, styles, ctx)));
    }

    if let Some(item) = content.to_packed::<EnumItem>() {
        return LatexIr::Item(Box::new(lower_content(&item.body, styles, ctx)));
    }

    if let Some(item) = content.to_packed::<TermItem>() {
        return LatexIr::Item(Box::new(lower_content(&item.description, styles, ctx)));
    }

    if let Some(block) = content.to_packed::<BlockElem>() {
        if let Some(body) = block.body.get_cloned(styles) {
            if let typst::layout::BlockBody::Content(c) = body {
                return LatexIr::Block(Box::new(lower_content(&c, styles, ctx)));
            }
        }
        return LatexIr::Sequence(vec![]);
    }

    if let Some(rect) = content.to_packed::<RectElem>() {
        if let Some(body) = rect.body.get_cloned(styles) {
            return LatexIr::Block(Box::new(lower_content(&body, styles, ctx)));
        }
        return LatexIr::Sequence(vec![]);
    }

    if let Some(raw) = content.to_packed::<RawElem>() {
        let lang = raw.lang.get_cloned(styles).map(|l| l.to_string());
        let text = match raw.text.clone() {
            typst::text::RawContent::Text(t) => t.to_string(),
            typst::text::RawContent::Lines(lines) => {
                let s_lines: Vec<_> = lines.into_iter().map(|(s, _)| s.to_string()).collect();
                s_lines.join("\n")
            }
        };
        return LatexIr::Raw(text, lang);
    }

    if let Some(link) = content.to_packed::<LinkElem>() {
        return LatexIr::Link("".to_string(), Some(Box::new(lower_content(&link.body, styles, ctx))));
    }

    if let Some(sq) = content.to_packed::<SmartQuoteElem>() {
        return LatexIr::SmartQuote(sq.double.get(styles));
    }

    if let Some(b) = content.to_packed::<BoxElem>() {
        let body_content = b.body.get_cloned(styles);
        if let Some(c) = body_content {
            return lower_content(&c, styles, ctx);
        }
        return LatexIr::Text("".to_string());
    }

    if let Some(_h) = content.to_packed::<HElem>() {
        return LatexIr::Space;
    }

    if let Some(a) = content.to_packed::<AlignElem>() {
        return lower_content(&a.body, styles, ctx);
    }

    if let Some(s) = content.to_packed::<StackElem>() {
        let mut children = Vec::new();
        for child in s.children.iter() {
            if let StackChild::Block(c) = child {
                children.push(lower_content(c, styles, ctx));
            }
        }
        let mut seq = Vec::new();
        for (i, child) in children.into_iter().enumerate() {
            if i > 0 {
                seq.push(LatexIr::Latex(" \\\\ ".to_string()));
            }
            seq.push(child);
        }
        return LatexIr::Sequence(seq);
    }

    if let Some(sym) = content.to_packed::<SymbolElem>() {
        return LatexIr::Text(sym.text.to_string());
    }

    // Invisible instrumentation (e.g. wordometer's `state.update()`): a pure
    // state mutation with no visual output. Drop it silently rather than
    // flagging it unsupported.
    if content.is::<StateUpdateElem>() {
        return LatexIr::Sequence(vec![]);
    }

    if let Some(footnote) = content.to_packed::<FootnoteElem>() {
        return match &footnote.body {
            FootnoteBody::Content(c) => LatexIr::Footnote(Box::new(lower_content(c, styles, ctx))),
            // A footnote that references an earlier one: reuse its mark.
            FootnoteBody::Reference(_) => LatexIr::Latex("\\footnotemark".to_string()),
        };
    }

    if let Some(strong) = content.to_packed::<StrongElem>() {
        return LatexIr::Strong(Box::new(lower_content(&strong.body, styles, ctx)));
    }

    if let Some(emph) = content.to_packed::<EmphElem>() {
        return LatexIr::Emph(Box::new(lower_content(&emph.body, styles, ctx)));
    }

    if let Some(reference) = content.to_packed::<RefElem>() {
        // `@key` is a cross-reference if `key` is a label defined in the
        // document, otherwise a citation into the bibliography.
        let key = reference.target.resolve().to_string();
        return if ctx.defined_labels.contains(&key) {
            LatexIr::Reference(key)
        } else {
            LatexIr::Cite(vec![key])
        };
    }

    if let Some(cite) = content.to_packed::<CiteElem>() {
        return LatexIr::Cite(vec![cite.key.resolve().to_string()]);
    }

    if let Some(group) = content.to_packed::<CiteGroup>() {
        let keys = group
            .children
            .iter()
            .filter_map(|c| c.to_packed::<CiteElem>().map(|e| e.key.resolve().to_string()))
            .collect();
        return LatexIr::Cite(keys);
    }

    if let Some(bib) = content.to_packed::<BibliographyElem>() {
        for ds in &bib.sources.source.0 {
            if let DataSource::Path(p) = ds {
                let name = match p {
                    PathOrStr::Path(rp) => rp.vpath().get_without_slash().to_string(),
                    PathOrStr::Str(s) => s.as_str().to_string(),
                };
                if !ctx.bib_resources.contains(&name) {
                    ctx.bib_resources.push(name);
                }
            }
        }
        return LatexIr::Latex("\\printbibliography\n".to_string());
    }

    if let Some(figure) = content.to_packed::<FigureElem>() {
        // theorion theorem environments evaluate to a `figure` whose body
        // carries a `<theorion-frame-metadata>` dict with the clean kind/
        // title/body. Recover those and emit an amsthm environment, skipping
        // the context-heavy rendered box that would otherwise be lowered.
        if let Some(env) = lower_theorion_frame(&figure.body, styles, ctx) {
            return env;
        }
        let body = Box::new(lower_content(&figure.body, styles, ctx));
        let caption = figure
            .caption
            .get_cloned(styles)
            .map(|c| Box::new(lower_content(&c.body, styles, ctx)));
        return LatexIr::Figure { body, caption };
    }

    if let Some(table) = content.to_packed::<TableElem>() {
        return lower_table(table, styles, ctx);
    }

    if let Some(metadata) = content.to_packed::<MetadataElem>() {
        if let Some(ir) = lower_curryst_metadata(metadata, styles, ctx) {
            return ir;
        }
        // fletcher diagram marker → tikz-cd.
        if let Value::Dict(dict) = &metadata.value {
            if matches!(dict.get(&typst::foundations::Str::from("type")).ok(), Some(Value::Str(t)) if t.as_str() == "fletcher-diagram") {
                if let Some(Value::Content(body)) = dict.get(&typst::foundations::Str::from("body")).ok() {
                    return crate::core::typst2latex::lower_fletcher::lower_diagram(body, styles, ctx);
                }
            }
        }
        return ctx.record_unsupported("metadata", content.span());
    }

    ctx.record_unsupported(content.elem().name(), content.span())
}

/// theorion theorem environments (`#theorem`, `#lemma`, `#definition`, ... —
/// anything built via theorion's `make-frame`) evaluate to a `figure` whose
/// body contains a `metadata((identifier, kind, title, body, ...))` dict tagged
/// `<theorion-frame-metadata>`. If `content` contains such a dict, emit the
/// corresponding amsthm environment from the clean `kind`/`title`/`body`.
fn lower_theorion_frame(content: &Content, styles: StyleChain, ctx: &mut LowerContext) -> Option<LatexIr> {
    let dict = find_theorion_frame(content)?;
    let kind = match dict.get(&typst::foundations::Str::from("kind")).ok()? {
        Value::Str(s) => s.as_str().to_string(),
        _ => return None,
    };
    let body = match dict.get(&typst::foundations::Str::from("body")).ok()? {
        Value::Content(c) => c.clone(),
        _ => return None,
    };
    // `title` is empty (Str "") when the environment was used without a title,
    // or arbitrary content otherwise.
    let title = match dict.get(&typst::foundations::Str::from("title")).ok() {
        Some(Value::Content(c)) => Some(Box::new(lower_content(c, styles, ctx))),
        Some(Value::Str(s)) if !s.is_empty() => Some(Box::new(LatexIr::Text(s.as_str().to_string()))),
        _ => None,
    };
    Some(LatexIr::TheoremEnv {
        env: kind,
        title,
        body: Box::new(lower_content(&body, styles, ctx)),
    })
}

/// Recursively search `content` for a theorion frame metadata dict (a metadata
/// element whose dict has `identifier` + `kind` + `body`). Only descends
/// through sequences/styled wrappers — it does not walk into dict values.
fn find_theorion_frame<'a>(content: &'a Content) -> Option<&'a typst::foundations::Dict> {
    if let Some(md) = content.to_packed::<MetadataElem>() {
        if let Value::Dict(dict) = &md.value {
            let has = |k| dict.get(&typst::foundations::Str::from(k)).is_ok();
            if has("identifier") && has("kind") && has("body") {
                return Some(dict);
            }
        }
        return None;
    }
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        return seq.children.iter().find_map(find_theorion_frame);
    }
    if let Some(styled) = content.to_packed::<StyledElem>() {
        return find_theorion_frame(&styled.child);
    }
    None
}

/// Lower a Typst `table(...)` into a simple LaTeX `tabular`. Handles the
/// common grid-of-cells case (column count from `columns`, cells in row-major
/// order); hlines/vlines and header/footer grouping collapse into a plain
/// bordered tabular.
fn lower_table(table: &typst::foundations::Packed<TableElem>, styles: StyleChain, ctx: &mut LowerContext) -> LatexIr {
    let cols = table.columns.get_ref(styles).0.len().max(1);

    let mut cells: Vec<LatexIr> = Vec::new();
    let collect_item = |item: &TableItem, ctx: &mut LowerContext, cells: &mut Vec<LatexIr>| {
        if let TableItem::Cell(cell) = item {
            cells.push(lower_content(&cell.body, styles, ctx));
        }
    };
    for child in table.children.iter() {
        match child {
            TableChild::Item(item) => collect_item(item, ctx, &mut cells),
            TableChild::Header(h) => {
                for item in h.children.iter() {
                    collect_item(item, ctx, &mut cells);
                }
            }
            TableChild::Footer(f) => {
                for item in f.children.iter() {
                    collect_item(item, ctx, &mut cells);
                }
            }
        }
    }

    let rows: Vec<Vec<LatexIr>> = cells
        .chunks(cols)
        .map(|chunk| chunk.to_vec())
        .collect();
    LatexIr::Table { cols, rows }
}

/// Lower the marker `metadata(...)` dictionaries emitted by the bundled
/// curryst compatibility shim (`packages/curryst.typ`). Returns `None` (never
/// panics) for any dict shape that doesn't match what the shim produces, so a
/// malformed/foreign metadata value degrades to `Unsupported` instead of
/// crashing the whole conversion.
fn lower_curryst_metadata<'s>(
    metadata: &typst::foundations::Packed<MetadataElem>,
    styles: StyleChain<'s>,
    ctx: &mut LowerContext,
) -> Option<LatexIr> {
    let Value::Dict(dict) = &metadata.value else { return None };
    let Value::Str(kind) = dict.get(&typst::foundations::Str::from("type")).ok()? else { return None };

    match kind.as_str() {
        "curryst-rule" => {
            let Value::Array(premises_array) = dict.get(&typst::foundations::Str::from("premises")).ok()? else { return None };
            let Value::Content(conclusion_content) = dict.get(&typst::foundations::Str::from("conclusion")).ok()? else { return None };

            let mut premises = Vec::new();
            for premise_val in premises_array.iter() {
                if let Value::Content(c) = premise_val {
                    premises.push(lower_content(c, styles, ctx));
                }
            }
            let conclusion = Box::new(lower_content(conclusion_content, styles, ctx));
            Some(LatexIr::InferenceRule { premises, conclusion })
        }
        "curryst-rule-set" => {
            let Value::Array(trees_array) = dict.get(&typst::foundations::Str::from("trees")).ok()? else { return None };
            let mut trees = Vec::new();
            for tree_val in trees_array.iter() {
                if let Value::Content(c) = tree_val {
                    trees.push(lower_content(c, styles, ctx));
                }
            }
            Some(LatexIr::RuleGroup(trees))
        }
        "curryst-prooftree" => {
            let Value::Content(rule_content) = dict.get(&typst::foundations::Str::from("rule")).ok()? else { return None };
            Some(lower_content(rule_content, styles, ctx))
        }
        _ => None,
    }
}
