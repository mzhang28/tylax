//! Lower *evaluated* Typst math content to LaTeX.
//!
//! Unlike the old approach (which re-read the equation's source span and
//! reparsed raw syntax), this walks the real evaluated/realized math element
//! tree produced by `typst_realize::realize(RealizationKind::Math, ...)`. That
//! is what lets constructs like `$config(a, b, c)$` (a variadic helper that
//! joins its arguments) or `$mobile(x)$` (a function-valued math macro) and
//! quick-maths `shorthands` work correctly: they are already expanded in the
//! evaluated content, so we never have to understand Typst scripting here.

use typst::foundations::{Content, Packed, StyleChain, SequenceElem, SymbolElem};
use typst::text::{TextElem, SpaceElem, LinebreakElem};
use typst::layout::{HElem, Spacing};
use typst::math::{EquationElem, AttachElem, FracElem, RootElem, LrElem, OpElem, AlignPointElem, PrimesElem, ScriptsElem, LimitsElem,
    OverlineElem, UnderlineElem, OverbraceElem, UnderbraceElem, OverbracketElem, UnderbracketElem, OverparenElem, UnderparenElem, OvershellElem, UndershellElem};
use typst::text::RawElem;
use codex::styling::MathVariant;

use crate::core::typst2latex::ir::LatexIr;
use crate::core::typst2latex::lower::LowerContext;
use crate::core::typst2latex::utils::UNICODE_TO_LATEX;

/// Lower an equation element to a `LatexIr::Math` node.
pub fn lower_equation(eq: &Packed<EquationElem>, styles: StyleChain, ctx: &mut LowerContext) -> LatexIr {
    let block = eq.block.get(styles);

    // Reconstruct the equation as `Content` so we can apply equation-level
    // user show rules (e.g. quick-maths `shorthands`, which install a
    // `show math.equation: ...` recipe) before structuring the math.
    let eq_content = eq.clone().pack();
    let transformed = apply_equation_recipes(eq_content, styles, ctx);

    let (inner, has_align) = render_math(&transformed, styles, ctx, eq.span());

    if block {
        if has_align {
            LatexIr::Math(format!("\\[\n\\begin{{aligned}}\n{inner}\n\\end{{aligned}}\n\\]"))
        } else {
            LatexIr::Math(format!("\\[\n{inner}\n\\]"))
        }
    } else {
        LatexIr::Math(format!("\\ensuremath{{{inner}}}"))
    }
}

/// Realize `content` with `RealizationKind::Math` and emit it to a raw LaTeX
/// math string (no `\[ \]` / `\ensuremath` wrapper). Returns the string and
/// whether it contains alignment/linebreaks. Unknown math elements are recorded
/// against `span`.
///
/// Realizing into a *local* arena keeps the borrow in this scope; we emit while
/// it is still alive. Math kind applies math show rules (incl. the nested
/// regex/sequence rules shorthands rely on) and yields clean structured math.
fn render_math(content: &Content, styles: StyleChain, ctx: &mut LowerContext, span: typst::syntax::Span) -> (String, bool) {
    let arenas = typst::routines::Arenas::default();
    let mut out = String::new();
    let mut has_align = false;
    let mut unsup: Vec<String> = Vec::new();
    match typst_realize::realize(
        typst::routines::RealizationKind::Math,
        &mut ctx.engine,
        &mut ctx.locator,
        &arenas,
        content,
        styles,
    ) {
        Ok(pairs) => {
            for (c, s) in &pairs {
                emit(c, *s, &mut out, &mut has_align, &mut unsup);
            }
        }
        Err(_) => emit(content, styles, &mut out, &mut has_align, &mut unsup),
    }
    for name in unsup {
        let _ = ctx.record_unsupported(&name, span);
    }
    (out.split_whitespace().collect::<Vec<_>>().join(" "), has_align)
}

/// Lower a piece of math content to a raw LaTeX math string (no wrapper). Used
/// for embedding math into other LaTeX contexts such as `tikz-cd` cells and
/// arrow labels.
pub fn lower_math_fragment(content: &Content, styles: StyleChain, ctx: &mut LowerContext) -> String {
    render_math(content, styles, ctx, content.span()).0
}

/// Apply any equation-level user show rules from the current style chain to the
/// equation content (returning the transformed content). Built-in layout show
/// rules live in `library.rules`, not the chain, so they are never applied
/// here — we only want user recipes such as quick-maths shorthands.
fn apply_equation_recipes(content: Content, styles: StyleChain, ctx: &mut LowerContext) -> Content {
    let recipes: Vec<_> = styles.recipes().cloned().collect();
    let mut current = content;
    // Innermost (closest) recipe first: `recipes()` yields outermost-first.
    for recipe in recipes.iter().rev() {
        if let Some(sel) = recipe.selector() {
            if sel.matches(&current, Some(styles)) {
                let context = typst::foundations::Context::new(None, Some(styles));
                if let Ok(new) = recipe.apply(&mut ctx.engine, typst::comemo::Track::track(&context), current.clone()) {
                    current = new;
                }
            }
        }
    }
    current
}

/// Recursively emit LaTeX for one math element into `out`. Unknown element
/// names are collected into `unsup` so the caller can enforce the unsupported
/// policy (they are also rendered as a visible marker).
fn emit(content: &Content, styles: StyleChain, out: &mut String, has_align: &mut bool, unsup: &mut Vec<String>) {
    if let Some(seq) = content.to_packed::<SequenceElem>() {
        for child in seq.children.iter() {
            emit(child, styles, out, has_align, unsup);
        }
        return;
    }

    // Introspection tags carry no visible output.
    if content.is::<typst::introspection::TagElem>() {
        return;
    }

    // A `block`/`box` wrapper (e.g. from a `[$…$]` content block used as a
    // fletcher node body): recurse into its body.
    if let Some(block) = content.to_packed::<typst::layout::BlockElem>() {
        if let Some(typst::layout::BlockBody::Content(c)) = block.body.get_cloned(styles) {
            emit(&c, styles, out, has_align, unsup);
        }
        return;
    }
    if let Some(b) = content.to_packed::<typst::layout::BoxElem>() {
        if let Some(c) = b.body.get_cloned(styles) {
            emit(&c, styles, out, has_align, unsup);
        }
        return;
    }

    if let Some(styled) = content.to_packed::<typst::foundations::StyledElem>() {
        let chained = styles.chain(&styled.styles);
        emit(&styled.child, chained, out, has_align, unsup);
        return;
    }

    // A nested equation (e.g. from interpolation) — recurse into its body.
    if let Some(eq) = content.to_packed::<EquationElem>() {
        emit(&eq.body, styles, out, has_align, unsup);
        return;
    }

    if let Some(sym) = content.to_packed::<SymbolElem>() {
        push_str_glyphs(&sym.text.to_string(), styles, out);
        return;
    }

    if let Some(text) = content.to_packed::<TextElem>() {
        emit_text(&text.text, styles, out);
        return;
    }

    if content.is::<SpaceElem>() {
        out.push(' ');
        return;
    }

    if let Some(h) = content.to_packed::<HElem>() {
        emit_kern(h, out);
        return;
    }

    if content.is::<LinebreakElem>() {
        out.push_str(" \\\\ ");
        *has_align = true;
        return;
    }

    if content.is::<AlignPointElem>() {
        out.push('&');
        *has_align = true;
        return;
    }

    if let Some(frac) = content.to_packed::<FracElem>() {
        out.push_str("\\frac{");
        emit(&frac.num, styles, out, has_align, unsup);
        out.push_str("}{");
        emit(&frac.denom, styles, out, has_align, unsup);
        out.push('}');
        return;
    }

    if let Some(root) = content.to_packed::<RootElem>() {
        out.push_str("\\sqrt");
        if let Some(index) = root.index.get_cloned(styles) {
            out.push('[');
            emit(&index, styles, out, has_align, unsup);
            out.push(']');
        }
        out.push('{');
        emit(&root.radicand, styles, out, has_align, unsup);
        out.push('}');
        return;
    }

    if let Some(attach) = content.to_packed::<AttachElem>() {
        emit_attach(attach, styles, out, has_align, unsup);
        return;
    }

    if let Some(lr) = content.to_packed::<LrElem>() {
        emit_lr(lr, styles, out, has_align, unsup);
        return;
    }

    if let Some(op) = content.to_packed::<OpElem>() {
        // Text operators like `op("foo")` / `sin` etc.
        let mut inner = String::new();
        let mut dummy = false;
        emit(&op.text, styles, &mut inner, &mut dummy, unsup);
        out.push_str(&format!("\\operatorname{{{}}}", inner.trim()));
        return;
    }

    if let Some(primes) = content.to_packed::<PrimesElem>() {
        for _ in 0..primes.count {
            out.push('\'');
        }
        return;
    }

    // `scripts()`/`limits()` only control attachment placement; for LaTeX we
    // just render the wrapped body.
    if let Some(scripts) = content.to_packed::<ScriptsElem>() {
        emit(&scripts.body, styles, out, has_align, unsup);
        return;
    }
    if let Some(limits) = content.to_packed::<LimitsElem>() {
        emit(&limits.body, styles, out, has_align, unsup);
        return;
    }

    // Over/under decorations. The brace/bracket/paren/shell variants carry an
    // optional annotation, rendered as a super/subscript on the accent.
    if let Some(e) = content.to_packed::<OverlineElem>() {
        return emit_accent("\\overline", &e.body, None, styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<UnderlineElem>() {
        return emit_accent("\\underline", &e.body, None, styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<OverbraceElem>() {
        return emit_accent("\\overbrace", &e.body, e.annotation.get_cloned(styles).map(|a| ('^', a)), styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<UnderbraceElem>() {
        return emit_accent("\\underbrace", &e.body, e.annotation.get_cloned(styles).map(|a| ('_', a)), styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<OverbracketElem>() {
        return emit_accent("\\overbracket", &e.body, e.annotation.get_cloned(styles).map(|a| ('^', a)), styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<UnderbracketElem>() {
        return emit_accent("\\underbracket", &e.body, e.annotation.get_cloned(styles).map(|a| ('_', a)), styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<OverparenElem>() {
        return emit_accent("\\overparen", &e.body, e.annotation.get_cloned(styles).map(|a| ('^', a)), styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<UnderparenElem>() {
        return emit_accent("\\underparen", &e.body, e.annotation.get_cloned(styles).map(|a| ('_', a)), styles, out, has_align, unsup);
    }
    // No standard LaTeX "shell" accent; approximate with a paren accent.
    if let Some(e) = content.to_packed::<OvershellElem>() {
        return emit_accent("\\overparen", &e.body, e.annotation.get_cloned(styles).map(|a| ('^', a)), styles, out, has_align, unsup);
    }
    if let Some(e) = content.to_packed::<UndershellElem>() {
        return emit_accent("\\underparen", &e.body, e.annotation.get_cloned(styles).map(|a| ('_', a)), styles, out, has_align, unsup);
    }

    // Inline raw (`` `code` ``) inside math → monospace.
    if let Some(raw) = content.to_packed::<RawElem>() {
        let text = match raw.text.clone() {
            typst::text::RawContent::Text(t) => t.to_string(),
            typst::text::RawContent::Lines(lines) => {
                lines.into_iter().map(|(s, _)| s.to_string()).collect::<Vec<_>>().join(" ")
            }
        };
        out.push_str(&format!("\\mathtt{{{}}}", text.replace('\\', "\\backslash ")));
        return;
    }

    // Unknown math element: mark it visibly and record it for the policy check.
    let name = content.elem().name();
    unsup.push(format!("math.{name}"));
    out.push_str(&format!(" \\mathrm{{[?{name}]}} "));
}

fn emit_attach(attach: &Packed<AttachElem>, styles: StyleChain, out: &mut String, has_align: &mut bool, unsup: &mut Vec<String>) {
    let tl = attach.tl.get_cloned(styles);
    let bl = attach.bl.get_cloned(styles);
    // Pre-scripts (rare): emit using an empty base sidebearing.
    if tl.is_some() || bl.is_some() {
        out.push_str("{}");
        if let Some(tl) = &tl { out.push('^'); emit_group(tl, styles, out, has_align, unsup); }
        if let Some(bl) = &bl { out.push('_'); emit_group(bl, styles, out, has_align, unsup); }
    }

    out.push('{');
    emit(&attach.base, styles, out, has_align, unsup);
    out.push('}');

    if let Some(b) = attach.b.get_cloned(styles) {
        out.push('_');
        emit_group(&b, styles, out, has_align, unsup);
    }
    if let Some(t) = attach.t.get_cloned(styles) {
        out.push('^');
        emit_group(&t, styles, out, has_align, unsup);
    }
    if let Some(br) = attach.br.get_cloned(styles) {
        out.push('_');
        emit_group(&br, styles, out, has_align, unsup);
    }
    if let Some(tr) = attach.tr.get_cloned(styles) {
        out.push('^');
        emit_group(&tr, styles, out, has_align, unsup);
    }
}

/// Emit an over/under accent (`\overline`, `\overbrace`, ...) around `body`,
/// with an optional annotation attached as a super/subscript (`pos` is `'^'`
/// or `'_'`).
fn emit_accent(cmd: &str, body: &Content, annotation: Option<(char, Content)>, styles: StyleChain, out: &mut String, has_align: &mut bool, unsup: &mut Vec<String>) {
    out.push_str(cmd);
    emit_group(body, styles, out, has_align, unsup);
    if let Some((pos, ann)) = annotation {
        out.push(pos);
        emit_group(&ann, styles, out, has_align, unsup);
    }
}

/// Emit content wrapped in `{...}` (for a sub/superscript group).
fn emit_group(content: &Content, styles: StyleChain, out: &mut String, has_align: &mut bool, unsup: &mut Vec<String>) {
    out.push('{');
    emit(content, styles, out, has_align, unsup);
    out.push('}');
}

fn emit_lr(lr: &Packed<LrElem>, styles: StyleChain, out: &mut String, has_align: &mut bool, unsup: &mut Vec<String>) {
    // The LrElem body is a sequence whose first/last elements are the opening
    // and closing delimiters. Render as `\left<open> ... \right<close>`.
    let body = &lr.body;
    let children: Vec<&Content> = if let Some(seq) = body.to_packed::<SequenceElem>() {
        seq.children.iter().collect()
    } else {
        vec![body]
    };

    let open = children.first().and_then(|c| delimiter_of(c));
    let close = children.last().and_then(|c| delimiter_of(c));

    match (open, close) {
        (Some(o), Some(c)) if children.len() >= 2 => {
            out.push_str(&format!("\\left{o} "));
            for child in &children[1..children.len() - 1] {
                emit(child, styles, out, has_align, unsup);
            }
            out.push_str(&format!(" \\right{c}"));
        }
        _ => {
            for child in &children {
                emit(child, styles, out, has_align, unsup);
            }
        }
    }
}

/// If this content is a single delimiter symbol, return its LaTeX form
/// (already `\`-prefixed where appropriate, or the raw char for `(` `)` `[` `]`).
fn delimiter_of(content: &Content) -> Option<String> {
    let text = if let Some(sym) = content.to_packed::<SymbolElem>() {
        sym.text.to_string()
    } else if let Some(t) = content.to_packed::<TextElem>() {
        t.text.to_string()
    } else {
        return None;
    };
    if text.chars().count() != 1 {
        return None;
    }
    let ch = text.chars().next().unwrap();
    match ch {
        '(' | ')' | '[' | ']' | '|' | '.' | '/' => Some(ch.to_string()),
        '{' => Some("\\{".to_string()),
        '}' => Some("\\}".to_string()),
        _ => UNICODE_TO_LATEX.get(&ch).map(|s| s.to_string()),
    }
}

/// Emit a run of glyphs (from a `SymbolElem`/delimiter string), mapping each
/// char to LaTeX and applying the active math variant.
fn push_str_glyphs(text: &str, styles: StyleChain, out: &mut String) {
    for ch in text.chars() {
        push_glyph(ch, styles, out);
    }
}

fn push_glyph(ch: char, styles: StyleChain, out: &mut String) {
    let mapped: String = match ch {
        '−' => "-".to_string(),          // U+2212 minus sign
        '\u{2062}' => return,            // invisible times
        '\u{2061}' => return,            // function application
        '\u{FE0E}' | '\u{FE0F}' => return, // variation selectors
        // Literal brace *characters* (Typst math braces are literal, not
        // grouping): escape so they don't act as LaTeX groups.
        '{' => "\\{".to_string(),
        '}' => "\\}".to_string(),
        _ => {
            if let Some(latex) = UNICODE_TO_LATEX.get(&ch) {
                format!("{latex} ")
            } else if ch.is_ascii_alphanumeric()
                || matches!(ch, '+' | '-' | '=' | '<' | '>' | '(' | ')' | '[' | ']' | '/' | '!' | '|' | ',' | '.' | ':' | ';' | '?')
            {
                ch.to_string()
            } else {
                // Unknown glyph: keep the literal char (LuaLaTeX/XeLaTeX via
                // tectonic + unicode-math would handle many of these).
                ch.to_string()
            }
        }
    };
    apply_variant(&mapped, styles, out);
}

/// Emit a text run (operator name / `sans("...")` content) as one unit,
/// applying the active variant.
fn emit_text(text: &str, styles: StyleChain, out: &mut String) {
    let escaped = text.replace('_', "\\_");
    apply_variant(&escaped, styles, out);
}

/// Wrap `body` in the LaTeX command for the current math variant/bold, if any.
fn apply_variant(body: &str, styles: StyleChain, out: &mut String) {
    let variant = styles.get(EquationElem::variant);
    let bold = styles.get(EquationElem::bold);
    let cmd = match variant {
        Some(MathVariant::SansSerif) => Some("\\mathsf"),
        Some(MathVariant::Chancery) | Some(MathVariant::Roundhand) => Some("\\mathcal"),
        Some(MathVariant::Fraktur) => Some("\\mathfrak"),
        Some(MathVariant::Monospace) => Some("\\mathtt"),
        Some(MathVariant::DoubleStruck) => Some("\\mathbb"),
        _ => None,
    };
    match (cmd, bold) {
        (Some(cmd), _) => out.push_str(&format!("{cmd}{{{}}}", body.trim())),
        (None, true) => out.push_str(&format!("\\mathbf{{{}}}", body.trim())),
        (None, false) => out.push_str(body),
    }
}

/// Emit a horizontal kern (`#h(...)` inside math). Negative kerns become
/// `\mkern` with a converted em value; small positive weak spacing is dropped.
fn emit_kern(h: &Packed<HElem>, out: &mut String) {
    if let Spacing::Rel(rel) = &h.amount {
        // Only the em (font-relative) part is meaningful as a math kern.
        let em = rel.abs.em.get();
        if em != 0.0 {
            // 1em ≈ 18mu in TeX math units.
            let mu = em * 18.0;
            out.push_str(&format!(" \\mkern{mu}mu "));
        }
    }
}
