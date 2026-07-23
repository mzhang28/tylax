#[derive(Debug, Clone)]
pub enum LatexIr {
    Document(Vec<LatexIr>),
    Sequence(Vec<LatexIr>),
    Heading { level: usize, content: Box<LatexIr> },
    Item(Box<LatexIr>),
    List(Vec<LatexIr>),
    NumberedList(Vec<LatexIr>),
    Raw(String, Option<String>),
    Link(String, Option<Box<LatexIr>>),
    SmartQuote(bool),
    Space,
    Text(String),
    Latex(String),
    Math(String),
    InferenceRule { premises: Vec<LatexIr>, conclusion: Box<LatexIr> },
    RuleGroup(Vec<LatexIr>),
    Parbreak,
    Block(Box<LatexIr>),
    Strong(Box<LatexIr>),
    Emph(Box<LatexIr>),
    Reference(String),
    Figure { body: Box<LatexIr>, caption: Option<Box<LatexIr>> },
    Table { cols: usize, rows: Vec<Vec<LatexIr>> },
    Unsupported(String),
}

/// Escape a text run for LaTeX: escape the reserved characters and translate a
/// few Unicode symbols/emoji that would otherwise emit "Missing character"
/// warnings (the source uses e.g. `#emoji.checkmark.box`). fontspec/XeTeX
/// (tectonic) handles the rest of Unicode text directly.
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\textbackslash "),
            '#' => out.push_str("\\#"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '_' => out.push_str("\\_"),
            '$' => out.push_str("\\$"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '^' => out.push_str("\\textasciicircum "),
            '~' => out.push_str("\\textasciitilde "),
            '\u{FE0F}' | '\u{FE0E}' => {} // variation selectors: drop
            '\u{2705}' | '\u{2714}' | '\u{2713}' => out.push_str("\\checkmark{}"), // check marks
            '…' => out.push_str("\\ldots{}"),
            _ => out.push(ch),
        }
    }
    out
}

impl LatexIr {
    pub fn render(&self) -> String {
        match self {
            Self::Document(children) | Self::Sequence(children) => {
                children.iter().map(|c| c.render()).collect::<Vec<_>>().join("")
            }
            Self::Heading { level, content } => {
                let cmd = match level {
                    1 => "section",
                    2 => "subsection",
                    3 => "subsubsection",
                    4 => "paragraph",
                    _ => "subparagraph",
                };
                format!("\\{cmd}{{{}}}\n", content.render())
            }
            Self::Item(content) => {
                format!("\\item {}\n", content.render())
            }
            Self::List(items) => {
                let inner = items.iter().map(|i| i.render()).collect::<Vec<_>>().join("");
                format!("\\begin{{itemize}}\n{}\\end{{itemize}}\n", inner)
            }
            Self::NumberedList(items) => {
                let inner = items.iter().map(|i| i.render()).collect::<Vec<_>>().join("");
                format!("\\begin{{enumerate}}\n{}\\end{{enumerate}}\n", inner)
            }
            Self::Raw(text, _lang) => {
                if text.contains('\n') {
                    format!("\\begin{{verbatim}}\n{}\n\\end{{verbatim}}\n", text)
                } else {
                    format!("\\verb|{}|", text.replace('|', "\\|"))
                }
            }
            Self::Link(url, display) => {
                if let Some(disp) = display {
                    format!("\\href{{{}}}{{{}}}", url, disp.render())
                } else {
                    format!("\\url{{{}}}", url)
                }
            }
            Self::SmartQuote(_) => "\"".to_string(),
            Self::Space => " ".to_string(),
            Self::Text(text) => escape_text(text),
            Self::Latex(latex) => latex.clone(),
            Self::Math(math) => math.clone(),
            Self::InferenceRule { premises, conclusion } => {
                let p = premises.iter().map(|p| p.render()).collect::<Vec<_>>().join(" \\\\ ");
                format!("\\inferrule{{{}}}{{{}}}", p, conclusion.render())
            }
            Self::RuleGroup(rules) => {
                let inner = rules.iter().map(|r| r.render()).collect::<Vec<_>>().join("\n\n");
                format!("\\begin{{mathpar}}\n{}\n\\end{{mathpar}}\n", inner)
            }
            Self::Parbreak => "\n\n".to_string(),
            Self::Block(content) => content.render(),
            Self::Strong(content) => format!("\\textbf{{{}}}", content.render()),
            Self::Emph(content) => format!("\\emph{{{}}}", content.render()),
            Self::Reference(label) => format!("\\ref{{{}}}", label),
            Self::Figure { body, caption } => {
                let cap = caption
                    .as_ref()
                    .map(|c| format!("\\caption{{{}}}\n", c.render()))
                    .unwrap_or_default();
                format!("\\begin{{figure}}[htbp]\n\\centering\n{}\n{}\\end{{figure}}\n", body.render(), cap)
            }
            Self::Table { cols, rows } => {
                let spec = "l".repeat((*cols).max(1));
                let mut body = String::new();
                for row in rows {
                    let cells: Vec<String> = row.iter().map(|c| c.render()).collect();
                    body.push_str(&cells.join(" & "));
                    body.push_str(" \\\\\n");
                }
                format!("\\begin{{tabular}}{{{}}}\n\\hline\n{}\\hline\n\\end{{tabular}}\n", spec, body)
            }
            Self::Unsupported(name) => format!("\\texttt{{[unsupported Typst element: {}]}}", name),
        }
    }
}
