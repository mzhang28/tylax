pub mod context;
pub mod world;
pub mod ir;
pub mod lower;
pub mod lower_math;
pub mod lower_fletcher;
pub mod utils;

pub use context::{ConvertContext, DocumentWrapperMode, EnvironmentContext, T2LOptions, TokenType, UnsupportedMode};
use crate::utils::error::CliDiagnostic;

#[derive(Debug, Clone)]
pub struct ConversionWarning {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ConversionResult {
    pub output: String,
    pub warnings: Vec<ConversionWarning>,
}

impl ConversionResult {
    pub fn ok(output: String) -> Self {
        Self { output, warnings: Vec::new() }
    }
}

impl From<ConversionWarning> for CliDiagnostic {
    fn from(warning: ConversionWarning) -> Self {
        CliDiagnostic::new(
            crate::utils::error::DiagnosticSeverity::Warning,
            "TypstConversionWarning".to_string(),
            warning.message,
        )
    }
}

/// A hard conversion failure: Typst source failed to load, parse, evaluate,
/// or realize. Per PLAN.md, a Typst evaluation failure must stop conversion
/// rather than emit plausible-looking but incorrect LaTeX.
#[derive(Debug, Clone)]
pub struct ConversionError {
    pub message: String,
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConversionError {}

impl From<ConversionError> for CliDiagnostic {
    fn from(err: ConversionError) -> Self {
        CliDiagnostic::new(
            crate::utils::error::DiagnosticSeverity::Error,
            "TypstConversionError".to_string(),
            err.message,
        )
    }
}

fn format_diagnostics(stage: &str, diags: &typst::diag::EcoVec<typst::diag::SourceDiagnostic>) -> ConversionError {
    let mut message = format!("Typst {stage} failed:\n");
    for diag in diags.iter() {
        message.push_str(&format!("  [{:?}] {}\n", diag.severity, diag.message));
        for hint in diag.hints.iter() {
            message.push_str(&format!("    hint: {}\n", hint.v));
        }
    }
    ConversionError { message }
}

pub fn convert(
    main_file_path: &std::path::Path,
    project_root: &std::path::Path,
    options: &T2LOptions,
) -> Result<ConversionResult, ConversionError> {
    use typst::World;
    use typst::engine::{Sink, Traced, Route};
    use typst::comemo::Track;

    let world = world::TylaxWorld::new(main_file_path, project_root);

    let library = world.library();
    let traced = Traced::default();
    let mut sink = Sink::new();
    let route = Route::default();

    let main_id = world.main();
    let main_source = world.source(main_id).map_err(|e| ConversionError {
        message: format!("failed to load main file {}: {:?}", main_file_path.display(), e),
    })?;

    let world_dyn: &dyn World = &world;
    let result = typst_eval::eval(
        world_dyn.track(),
        library,
        traced.track(),
        sink.track_mut(),
        route.track(),
        &main_source
    );

    let module = result.map_err(|diags| format_diagnostics("evaluation", &diags))?;
    let content = module.content();

    let target = typst_library::foundations::TargetElem::target.set(typst_library::foundations::Target::Paged).wrap();
    let base = typst_library::foundations::StyleChain::new(&library.styles);
    let styles = base.chain(&target);
    let mut sink2 = typst::engine::Sink::new();
    let introspector_impl = typst::introspection::EmptyIntrospector;
    let introspector: &dyn typst::introspection::Introspector = &introspector_impl;
    let constraint = comemo::Constraint::new();
    let traced2 = typst::engine::Traced::default();
    let engine = typst::engine::Engine {
        library: world_dyn.library(),
        world: world_dyn.track(),
        introspector: typst_utils::Protected::new(introspector.track_with(&constraint)),
        traced: traced2.track(),
        sink: sink2.track_mut(),
        route: typst::engine::Route::default(),
    };
    let arenas = typst::routines::Arenas::default();
    let locator = typst::introspection::Locator::root().split();

    // NOTE ON REALIZATION STRATEGY
    //
    // We deliberately do *not* run a document-level `typst_realize::realize`
    // here. Full document realization applies Typst's built-in layout show
    // rules, which destroy exactly the semantic structure we need for LaTeX:
    // equations collapse into opaque `InlineElem` layout callbacks and
    // headings (under a user `#show heading: ...` rule) collapse into styled
    // text blocks, losing the `HeadingElem`.
    //
    // Instead `lower::lower_content` walks the *evaluated* content tree while
    // threading the real `StyleChain` (so `#set` rules and math show-recipes
    // are in scope), and `lower_math` realizes only individual equation
    // bodies with `RealizationKind::Math` (after applying matching
    // equation-level user recipes such as quick-maths `shorthands`). This
    // keeps document structure intact while still honoring show/set rules.
    // Pre-pass: collect every label defined in the document so lowering can
    // distinguish a cross-reference (`@key` → defined label) from a citation
    // (`@key` → bibliography entry).
    let mut defined_labels = std::collections::HashSet::new();
    let _ = content.traverse(&mut |el: typst::foundations::Content| -> std::ops::ControlFlow<()> {
        if let Some(label) = el.label() {
            defined_labels.insert(label.resolve().to_string());
        }
        std::ops::ControlFlow::Continue(())
    });

    let mut ctx = lower::LowerContext {
        world: world_dyn,
        engine,
        locator,
        arenas: &arenas,
        styles,
        unsupported: Vec::new(),
        defined_labels,
        bib_resources: Vec::new(),
        in_table_cell: false,
    };

    let latex_ir = ir::LatexIr::Document(vec![lower::lower_content(&content, styles, &mut ctx)]);

    // Enforce the unsupported-construct policy. Per PLAN.md the default is to
    // stop hard so unsupported constructs can never silently vanish.
    if !ctx.unsupported.is_empty() {
        match options.unsupported {
            context::UnsupportedMode::Error => {
                let mut message = format!(
                    "{} unsupported Typst construct(s) encountered (use --unsupported=raw to emit visible markers instead):\n",
                    ctx.unsupported.len()
                );
                for u in &ctx.unsupported {
                    let loc = u.location.as_deref().unwrap_or("<unknown location>");
                    let pkg = u.package.as_deref().map(|p| format!(" [{p}]")).unwrap_or_default();
                    message.push_str(&format!("  - {} at {}{}\n", u.name, loc, pkg));
                }
                return Err(ConversionError { message });
            }
            context::UnsupportedMode::Image => {
                return Err(ConversionError {
                    message: "--unsupported=image is not yet implemented; use error or raw".to_string(),
                });
            }
            context::UnsupportedMode::Raw => {
                // Visible `\texttt{...}` markers are emitted by the IR renderer.
            }
        }
    }

    // Preamble: fontspec (XeTeX/LuaTeX via tectonic) + Libertinus Sans for
    // headings (mirrors the source's `#show heading: text(font: "Libertinus
    // Sans", ...)`), amsmath/mathtools/stmaryrd/mathpartir for the math and
    // inference rules.
    let mut preamble = concat!(
        "\\documentclass{article}\n",
        "\\usepackage{fontspec}\n",
        "\\usepackage{amsmath,amssymb}\n",
        "\\usepackage{mathtools}\n",
        "\\usepackage{stmaryrd}\n",
        "\\usepackage{mathpartir}\n",
        "\\usepackage{tikz-cd}\n",
        "\\usepackage{amsthm}\n",
        // Theorem environments (theorion's make-frame identifiers). `plain`
        // style for theorem-like, `definition` (upright) for the rest.
        "\\theoremstyle{plain}\n",
        "\\newtheorem{theorem}{Theorem}\n",
        "\\newtheorem{lemma}{Lemma}\n",
        "\\newtheorem{corollary}{Corollary}\n",
        "\\newtheorem{proposition}{Proposition}\n",
        "\\newtheorem{conjecture}{Conjecture}\n",
        "\\theoremstyle{definition}\n",
        "\\newtheorem{definition}{Definition}\n",
        "\\newtheorem{example}{Example}\n",
        "\\newtheorem{remark}{Remark}\n",
        "\\newtheorem{axiom}{Axiom}\n",
        "\\newtheorem{postulate}{Postulate}\n",
        "\\newtheorem{assumption}{Assumption}\n",
        "\\newtheorem{property}{Property}\n",
        "\\newtheorem{exercise}{Exercise}\n",
        "\\usepackage{titlesec}\n",
        "\\newfontfamily\\headingfont{Libertinus Sans}\n",
        "\\titleformat*{\\section}{\\Large\\bfseries\\headingfont}\n",
        "\\titleformat*{\\subsection}{\\large\\bfseries\\headingfont}\n",
        "\\titleformat*{\\subsubsection}{\\normalsize\\bfseries\\headingfont}\n",
        "\\usepackage{hyperref}\n",
    ).to_string();

    // Page geometry: paper size (`#set page(width/height)`) and a uniform
    // margin, folded into a single `geometry` package call.
    let mut geometry_opts: Vec<String> = Vec::new();
    let (page_w, page_h) = extract::get_page_size(&content);
    if let Some(w) = page_w {
        geometry_opts.push(format!("paperwidth={w}pt"));
    }
    if let Some(h) = page_h {
        geometry_opts.push(format!("paperheight={h}pt"));
    }
    if let Some(sides) = extract::get_page_margin(&content) {
        // Uniform margin (all four sides equal and concrete) → `margin=..pt`.
        if sides.left == sides.right && sides.top == sides.bottom && sides.left == sides.top {
            if let Some(typst::foundations::Smart::Custom(rel)) = sides.left {
                geometry_opts.push(format!("margin={}pt", rel.abs.abs.to_pt()));
            }
        }
    }
    if !geometry_opts.is_empty() {
        preamble.push_str(&format!("\\usepackage[{}]{{geometry}}\n", geometry_opts.join(",")));
    }

    if let Some(num) = extract::get_page_numbering(&content) {
        if num.contains("Arabic") {
            preamble.push_str("\\pagenumbering{arabic}\n");
        }
    }

    // Bibliography. Use biblatex's `bibtex` backend: tectonic has a built-in
    // bibtex engine (biber would be an external binary). The `.bib` file(s)
    // must sit alongside the generated `.tex` to compile.
    if !ctx.bib_resources.is_empty() {
        preamble.push_str("\\usepackage[backend=bibtex]{biblatex}\n");
        for res in &ctx.bib_resources {
            preamble.push_str(&format!("\\addbibresource{{{res}}}\n"));
        }
    }

    preamble.push_str("\\begin{document}\n\n");

    let final_latex = if options.full_document && matches!(options.wrapper, DocumentWrapperMode::Default) {
        let mut out = preamble;
        out.push_str(&latex_ir.render());
        out.push_str("\n\\end{document}\n");
        out
    } else {
        latex_ir.render()
    };

    Ok(ConversionResult::ok(final_latex))
}

pub mod extract;
