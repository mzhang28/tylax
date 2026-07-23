//! # tylax
//!
//! High-performance bidirectional LaTeX ↔ Typst converter written in Rust.
//!
//! ## Features
//!
//! - **High Performance**: AST-based parsing engine built on Rust
//! - **Bidirectional**: Supports both LaTeX → Typst and Typst → LaTeX
//! - **Full Document**: Converts complete documents including headings, lists, tables
//! - **Rich Symbol Set**: 700+ symbol mappings
//! - **WASM Support**: Compiles to WebAssembly for browser usage
//! - **Table Support**: Full table conversion with multicolumn/multirow
//! - **Reference System**: Complete citation and cross-reference support
//! - **Macro Expansion**: Basic LaTeX macro definition and expansion
//!
//! ## Usage Examples
//!
//! ### LaTeX → Typst (string based)
//!
//! ```rust
//! use tylax::latex_to_typst;
//!
//! let typst = latex_to_typst(r"\frac{1}{2}");
//! assert!(typst.contains("frac") || typst.contains("/"));
//! ```
//!
//! ### Typst → LaTeX (file based)
//!
//! Typst → LaTeX conversion runs the real Typst compiler (evaluation +
//! realization), so it operates on a file path + project root rather than a
//! raw source string (this is what makes imports, packages, and `#show`/`#set`
//! rules work). See [`convert`] and [`T2LOptions`]:
//!
//! ```no_run
//! use std::path::Path;
//! use tylax::{convert, T2LOptions};
//!
//! let result = convert(
//!     Path::new("paper/main.typ"),
//!     Path::new("paper"),
//!     &T2LOptions { full_document: true, ..Default::default() },
//! )
//! .expect("conversion failed");
//! println!("{}", result.output);
//! ```

/// Core conversion modules
pub mod core;

/// Data layer - static mappings and constants
pub mod data;

/// Feature modules - advanced conversion features
pub mod features;

/// Utility modules
pub mod utils;

/// Filesystem batch conversion API (native targets only)
#[cfg(not(target_arch = "wasm32"))]
pub mod batch;

/// WASM bindings (feature-gated)
#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export core conversion functions
pub use core::typst2latex;
pub use core::typst2latex::{
    convert, ConversionResult as T2LConversionResult,

};
pub use core::typst2latex::{DocumentWrapperMode, T2LOptions, UnsupportedMode};

pub use core::latex2typst::{
    convert_document_with_ast, convert_document_with_ast_options, convert_math_with_ast,
    convert_math_with_ast_options, convert_with_ast, convert_with_ast_options,
    latex_math_to_typst_with_diagnostics, latex_math_to_typst_with_eval,
    latex_to_typst_with_diagnostics, latex_to_typst_with_diagnostics_options,
    latex_to_typst_with_eval, ConversionMode, ConversionResult as L2TConversionResult,
    ConversionState, EnvironmentContext, L2TOptions, LatexConverter, PreambleMode, WarningKind,
};

// Re-export data modules
pub use data::constants;
pub use data::maps;

// Re-export feature modules
pub use features::bibtex;
pub use features::images;
pub use features::refs;
pub use features::tables;
pub use features::templates;
pub use features::tikz;

// Re-export symbol data
pub use data::colors;
pub use data::extended_symbols;
pub use data::physics;
pub use data::siunitx;
pub use data::symbols;

// Re-export utilities
pub use utils::diagnostics;
pub use utils::error::{
    CliDiagnostic, ConversionError, ConversionOutput, ConversionResult, ConversionWarning,
    DiagnosticSeverity,
};
pub use utils::files;

// Re-export main types and functions from eval (MiniEval) - now located in typst2latex

/// Convert LaTeX math code to Typst math code
///
/// # Arguments
/// * `input` - LaTeX math code
///
/// # Returns
/// Typst math code
pub fn latex_to_typst(input: &str) -> String {
    convert_math_with_ast(input)
}

/// Convert LaTeX math code to Typst math code with custom options
///
/// # Arguments
/// * `input` - LaTeX math code
/// * `options` - Conversion options
///
/// # Returns
/// Typst math code
pub fn latex_to_typst_with_options(input: &str, options: &L2TOptions) -> String {
    convert_math_with_ast_options(input, options.clone())
}

/// Convert a complete LaTeX document to Typst
pub fn latex_document_to_typst(input: &str) -> String {
    convert_document_with_ast(input)
}

/// Convert a complete LaTeX document to Typst with custom options
pub fn latex_document_to_typst_with_options(input: &str, options: &L2TOptions) -> String {
    convert_document_with_ast_options(input, options.clone())
}

/// Convert with automatic direction detection
///
/// Detects whether the input is LaTeX or Typst and converts accordingly.
/// Uses heuristics based on command patterns to determine the format.
pub fn convert_auto(input: &str) -> (String, &'static str) {
    // Heuristic: if input contains backslash commands, it's likely LaTeX
    let is_latex = input.contains('\\')
        && (input.contains("\\frac")
            || input.contains("\\alpha")
            || input.contains("\\sum")
            || input.contains("\\int")
            || input.contains("\\begin")
            || input.contains("\\section")
            || input.contains("\\documentclass"));

    if is_latex {
        (latex_to_typst(input), "typst")
    } else {
        (String::new(), "latex")
    }
}

/// Convert with automatic direction detection for full documents
pub fn convert_auto_document(input: &str) -> (String, &'static str) {
    let is_latex = input.contains("\\documentclass")
        || input.contains("\\begin{document}")
        || (input.contains('\\') && (input.contains("\\section") || input.contains("\\chapter")));

    let is_typst = input.contains("#set")
        || input.contains("#show")
        || input.starts_with('=')
        || input.contains("\n=");

    if is_latex && !is_typst {
        (latex_document_to_typst(input), "typst")
    } else if is_typst && !is_latex {
        (String::new(), "latex")
    } else if is_latex {
        (latex_document_to_typst(input), "typst")
    } else {
        (String::new(), "latex")
    }
}

/// Detect input format
///
/// Returns "latex", "typst", or "unknown" based on content analysis.
pub fn detect_format(input: &str) -> &'static str {
    // Strong LaTeX indicators
    let latex_score: i32 = if input.contains("\\documentclass") {
        10
    } else {
        0
    } + if input.contains("\\begin{document}") {
        10
    } else {
        0
    } + if input.contains("\\section") { 5 } else { 0 }
        + if input.contains("\\frac") { 3 } else { 0 }
        + if input.contains("\\alpha") { 2 } else { 0 }
        + if input.contains("\\\\") { 2 } else { 0 }
        + (input.matches('\\').count() as i32);

    // Strong Typst indicators
    let typst_score: i32 = if input.contains("#set") { 10 } else { 0 }
        + if input.contains("#show") { 10 } else { 0 }
        + if input.contains("#import") { 8 } else { 0 }
        + if input.starts_with('=') { 5 } else { 0 }
        + if input.contains("\n= ") { 5 } else { 0 }
        + if input.contains("frac(") { 3 } else { 0 }
        + if input.contains("sqrt(") { 3 } else { 0 };

    if latex_score > typst_score + 3 {
        "latex"
    } else if typst_score > latex_score + 3 {
        "typst"
    } else if latex_score > 0 {
        "latex"
    } else if typst_score > 0 {
        "typst"
    } else {
        "unknown"
    }
}

