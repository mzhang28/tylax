//! Unicode math symbol -> LaTeX command table.
//!
//! The only surviving piece of the old syntax-based converter: a char->LaTeX
//! map consumed by `lower_math` when lowering evaluated math content.

use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    /// Unicode math characters to LaTeX command mapping
    pub static ref UNICODE_TO_LATEX: HashMap<char, &'static str> = {
        let mut m = HashMap::new();

        // Greek lowercase
        m.insert('α', "\\alpha");
        m.insert('β', "\\beta");
        m.insert('γ', "\\gamma");
        m.insert('δ', "\\delta");
        m.insert('ε', "\\varepsilon");
        m.insert('ϵ', "\\epsilon");
        m.insert('ζ', "\\zeta");
        m.insert('η', "\\eta");
        m.insert('θ', "\\theta");
        m.insert('ϑ', "\\vartheta");
        m.insert('ι', "\\iota");
        m.insert('κ', "\\kappa");
        m.insert('λ', "\\lambda");
        m.insert('μ', "\\mu");
        m.insert('ν', "\\nu");
        m.insert('ξ', "\\xi");
        m.insert('π', "\\pi");
        m.insert('ρ', "\\rho");
        m.insert('ϱ', "\\varrho");
        m.insert('σ', "\\sigma");
        m.insert('ς', "\\varsigma");
        m.insert('τ', "\\tau");
        m.insert('υ', "\\upsilon");
        m.insert('φ', "\\varphi");
        m.insert('ϕ', "\\phi");
        m.insert('χ', "\\chi");
        m.insert('ψ', "\\psi");
        m.insert('ω', "\\omega");

        // Greek uppercase
        m.insert('Α', "A");
        m.insert('Β', "B");
        m.insert('Γ', "\\Gamma");
        m.insert('Δ', "\\Delta");
        m.insert('Ε', "E");
        m.insert('Ζ', "Z");
        m.insert('Η', "H");
        m.insert('Θ', "\\Theta");
        m.insert('Ι', "I");
        m.insert('Κ', "K");
        m.insert('Λ', "\\Lambda");
        m.insert('Μ', "M");
        m.insert('Ν', "N");
        m.insert('Ξ', "\\Xi");
        m.insert('Ο', "O");
        m.insert('Π', "\\Pi");
        m.insert('Ρ', "P");
        m.insert('Σ', "\\Sigma");
        m.insert('Τ', "T");
        m.insert('Υ', "\\Upsilon");
        m.insert('Φ', "\\Phi");
        m.insert('Χ', "X");
        m.insert('Ψ', "\\Psi");
        m.insert('Ω', "\\Omega");

        // Common math symbols
        m.insert('∞', "\\infty");
        m.insert('∂', "\\partial");
        m.insert('∇', "\\nabla");
        m.insert('∈', "\\in");
        m.insert('∉', "\\notin");
        m.insert('∋', "\\ni");
        m.insert('∅', "\\emptyset");
        m.insert('∀', "\\forall");
        m.insert('∃', "\\exists");
        m.insert('¬', "\\neg");
        m.insert('∧', "\\land");
        m.insert('∨', "\\lor");
        m.insert('∩', "\\cap");
        m.insert('∪', "\\cup");
        m.insert('⊂', "\\subset");
        m.insert('⊃', "\\supset");
        m.insert('⊆', "\\subseteq");
        m.insert('⊇', "\\supseteq");
        m.insert('×', "\\times");
        m.insert('÷', "\\div");
        m.insert('±', "\\pm");
        m.insert('∓', "\\mp");
        m.insert('·', "\\cdot");
        m.insert('∘', "\\circ");
        m.insert('⊕', "\\oplus");
        m.insert('⊗', "\\otimes");
        m.insert('⊸', "\\multimap");
        m.insert('⊎', "\\uplus");
        m.insert('⋆', "\\star");
        m.insert('⋅', "\\cdot");
        m.insert('⩴', "\\Coloneqq");
        m.insert('⟦', "\\llbracket");
        m.insert('⟧', "\\rrbracket");
        m.insert('†', "\\dagger");
        m.insert('‡', "\\ddagger");
        m.insert('★', "\\star");

        // Relations
        m.insert('≠', "\\neq");
        m.insert('≈', "\\approx");
        m.insert('≡', "\\equiv");
        m.insert('≤', "\\leq");
        m.insert('≥', "\\geq");
        m.insert('≪', "\\ll");
        m.insert('≫', "\\gg");
        m.insert('≺', "\\prec");
        m.insert('≻', "\\succ");
        m.insert('∼', "\\sim");
        m.insert('≃', "\\simeq");
        m.insert('≅', "\\cong");
        m.insert('∝', "\\propto");
        m.insert('⊥', "\\perp");
        m.insert('∥', "\\parallel");
        m.insert('⊢', "\\vdash");
        m.insert('⊣', "\\dashv");
        m.insert('⊨', "\\models");

        // Arrows
        m.insert('→', "\\rightarrow");
        m.insert('←', "\\leftarrow");
        m.insert('↔', "\\leftrightarrow");
        m.insert('⇒', "\\Rightarrow");
        m.insert('⇐', "\\Leftarrow");
        m.insert('⇔', "\\Leftrightarrow");
        m.insert('↦', "\\mapsto");
        m.insert('↑', "\\uparrow");
        m.insert('↓', "\\downarrow");
        m.insert('↗', "\\nearrow");
        m.insert('↘', "\\searrow");
        m.insert('↙', "\\swarrow");
        m.insert('↖', "\\nwarrow");
        m.insert('⟶', "\\longrightarrow");
        m.insert('⟵', "\\longleftarrow");
        m.insert('⟹', "\\Longrightarrow");
        m.insert('⟸', "\\Longleftarrow");

        // Big operators
        m.insert('∑', "\\sum");
        m.insert('∏', "\\prod");
        m.insert('∫', "\\int");
        m.insert('∬', "\\iint");
        m.insert('∭', "\\iiint");
        m.insert('∮', "\\oint");
        m.insert('⋂', "\\bigcap");
        m.insert('⋃', "\\bigcup");
        m.insert('⋀', "\\bigwedge");
        m.insert('⋁', "\\bigvee");

        // Delimiters
        m.insert('⟨', "\\langle");
        m.insert('⟩', "\\rangle");
        m.insert('⌈', "\\lceil");
        m.insert('⌉', "\\rceil");
        m.insert('⌊', "\\lfloor");
        m.insert('⌋', "\\rfloor");
        m.insert('‖', "\\|");

        // Dots
        m.insert('…', "\\ldots");
        m.insert('⋯', "\\cdots");
        m.insert('⋮', "\\vdots");
        m.insert('⋱', "\\ddots");

        // Misc
        m.insert('ℕ', "\\mathbb{N}");
        m.insert('ℤ', "\\mathbb{Z}");
        m.insert('ℚ', "\\mathbb{Q}");
        m.insert('ℝ', "\\mathbb{R}");
        m.insert('ℂ', "\\mathbb{C}");
        m.insert('ℓ', "\\ell");
        m.insert('ℏ', "\\hbar");
        m.insert('℘', "\\wp");
        m.insert('ℑ', "\\Im");
        m.insert('ℜ', "\\Re");
        m.insert('ℵ', "\\aleph");
        m.insert('□', "\\square");
        m.insert('◇', "\\diamond");
        m.insert('△', "\\triangle");
        m.insert('▽', "\\triangledown");
        m.insert('♠', "\\spadesuit");
        m.insert('♥', "\\heartsuit");
        m.insert('♦', "\\diamondsuit");
        m.insert('♣', "\\clubsuit");
        m.insert('′', "'");
        m.insert('″', "''");
        m.insert('°', "^\\circ");

        m
    };
}
