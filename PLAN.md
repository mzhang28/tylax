# Tylax Typst→LaTeX — support roadmap for `algebra.typ`

`tests/fixtures/algebra/main.typ` (a course-notes chapter) is the primary
extraction target. It evaluates and converts today (in `--unsupported=raw`),
but **146 constructs** hit the unsupported path. This document records what
each is and the change needed to support it.

## Current unsupported inventory

| Count | Construct | Origin | What it is |
|------:|-----------|--------|------------|
| 73 | `metadata` | theorion | theorem-counter markers |
| 49 | `context` | theorion | theorem numbering / title display |
| 10 | `context` | fletcher | commutative diagrams |
| 7  | `math.raw` | user | `` `code` `` inside `$…$` |
| 2  | `math.overline` | user | `overline(…)` in math |
| 2  | `context` | user (`main.typ:67`) | `if target() != "html"` branch |
| 1  | `footnote` | user | `#footnote[…]` |
| 1  | `bibliography` | user | `#bibliography("zotero.bib")` |
| 1  | `state-update` | wordometer | invisible word-count state |

## Root cause: unresolved `context`

Most failures (theorion, fletcher, wordometer) come from **`context`
expressions**. `ContextElem` wraps an internal `Func` that must run against
document state (counters, locations) — exactly what the realization step we
deliberately skip (see the memory note on the realization strategy) would
provide. Three strategies apply, chosen per feature:

1. **Semantic adapter shims** — replace a package's constructors with
   marker-emitting versions that sidestep `context` (as `packages/curryst.typ`
   already does). Best for theorion theorem environments.
2. **Image fallback** (`--unsupported=image`) — render a subtree via the
   `typst` binary and `\includegraphics` it. Best for diagrams; general escape
   hatch.
3. **Evaluate `context`** — invoke the func in a minimal engine context.
   Hard/risky; deferred.

## Work items (priority order)

### 1. Cheap math additions — `lower_math.rs`  ✅ DONE
- `typst::math::{OverlineElem, UnderlineElem}` → `\overline{}` / `\underline{}`.
- `math/underover.rs` family (`Over/Underbrace`, `Over/Underbracket`,
  `Over/Underparen`, `Over/Undershell`) with optional `annotation` →
  `\overbrace{body}^{ann}` etc.
- `RawElem` inside math → `\mathtt{…}`.

### 2. Footnotes + invisible instrumentation — `lower.rs` / `ir.rs`  ✅ DONE
- `FootnoteElem` → `LatexIr::Footnote` → `\footnote{body}`
  (`FootnoteBody::Content`; `Reference` → `\footnotemark`).
- Drop `typst::introspection::StateUpdateElem` silently (invisible counter
  mutation), not flagged unsupported.

### 3. Theorem environments (theorion) — biggest win, 122/146
`algebra` uses `#definition`(24), `#proof`(13), `#theorem`(8), `#exercise`(8),
`#lemma`(4), `#remark`, `#example`. Recommended: a **theorion adapter shim**
modelled on `packages/curryst.typ`, covering the imported entry points
(`theorion:*`, `cosmos.rainbow:*`, `show-theorion`, and the 7 constructors),
each emitting `metadata((kind, title, body))`. `lower.rs` maps these to
`\begin{theorem}[title]…\end{theorem}` with `amsthm` + `\newtheorem` decls in
the preamble. Generalize the `World::source` shim table (currently keyed on
`spec.name == "curryst"`).

### 4. Citations, cross-references, bibliography
`algebra` mixes `@key` **citations** (→ `zotero.bib`) with `@key`
**cross-refs** (→ `<label>`s). Today all `RefElem`s become `\ref{}` and **no
`\label{}` is ever emitted**, so nothing resolves. Needed:
- `CiteElem`/`CiteGroup` → `\cite{key}`; distinguish cite vs ref by whether the
  target resolves to a bib entry.
- Emit `\label{…}` from `elem.label()` on headings / equations / theorems.
- `BibliographyElem` → `biblatex` + `\printbibliography`; copy the `.bib`.

### 5. Diagrams (fletcher + cetz) — implement `--unsupported=image`
~30 diagram sites. Finish the plan's image mode: render the diagram subtree via
`typst` to SVG/PDF and emit `\includegraphics`. Doubles as the general fallback
for context-bound subtrees.

## Cross-cutting gaps
- **`\label` emission** missing entirely (blocks `\ref`/`\cite`).
- **Numbered equations**: `#set math.equation(numbering:"(1)")` — currently
  unnumbered `\[…\]`; should use `equation`/`align` for numbered, referenceable
  math.
- **Page geometry**: `extract.rs` reads margin/numbering but not
  `#set page(width/height)`.
