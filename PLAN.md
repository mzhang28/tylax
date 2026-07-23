# Tylax Typst→LaTeX — support roadmap for `algebra.typ`

`tests/fixtures/algebra/main.typ` (a course-notes chapter) is the primary
extraction target. It evaluates, converts, and compiles under tectonic. The
unsupported count is down from **146 → 14** (items 1–5 + page geometry done);
the remainder is `#proof` and one user `context`.

## Remaining unsupported inventory (14)

| Count | Construct | Origin | What it is |
|------:|-----------|--------|------------|
| 12 | `context` | theorion | `#proof[…]` (`context` for QED/noanswer) |
| 2  | `context` | user (`main.typ:67`) | `if target() != "html"` branch |

Both are `context` expressions — resolving them needs context evaluation
(invoke the `ContextElem` func in a minimal engine context), which is the next
frontier. `#proof` would then map to amsthm's `proof` environment.

Done this cycle: math raw/overline/decorations (1), footnote + state-update (2),
theorion theorem environments (3), citations/cross-refs/bibliography/labels (4),
fletcher diagrams → tikz-cd (5), and page geometry (`#set page(width/height)`).

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

### 3. Theorem environments (theorion) → amsthm  ✅ DONE
Instead of a shim: theorion's `make-frame` environments evaluate to a
`figure(kind: …)` whose body carries a `<theorion-frame-metadata>` dict with
`kind`/`title`/`body`. `lower.rs` detects that dict, emits `LatexIr::TheoremEnv`
→ `\begin{kind}[title]…\end{kind}`, and skips the context-heavy rendered box.
`amsthm` + `\newtheorem` decls added to the preamble. (`#proof` is separate — a
bare `context`, still unsupported; `#exercise`/`#example` already lower as plain
`emph`+body.)

### 4. Citations, cross-references, bibliography  ✅ DONE
- Pre-pass collects all document labels; `RefElem` → `\ref` if the target is a
  defined label, else `\cite`. `CiteElem`/`CiteGroup` → `\cite{keys}`.
- A `\label{…}` is emitted after any labelled element (wrapper in
  `lower_content`).
- `BibliographyElem` → `\printbibliography`; preamble gets
  `\usepackage[backend=bibtex]{biblatex}` + `\addbibresource{…}` (bibtex, not
  biber: tectonic has a built-in bibtex engine). The `.bib` must sit alongside
  the generated `.tex`.

### 5. Diagrams (fletcher) → tikz-cd  ✅ DONE
A bundled `packages/fletcher.typ` shim replaces fletcher's entrypoint
(`src/exports.typ`), capturing `diagram`/`node`/`edge` as `metadata` markers
(the `World::source` shim table now covers curryst + fletcher). `lower_fletcher`
lowers those to `tikz-cd`, handling both the matrix style (cells + attached
`edge("r"/"d"/…)` markers → `\arrow[dir, "label"]`, with `equal`/`leftarrow`/…
arrow styles and `'` swap for label-side) and the coordinate style
(`node((x,y),…)` + `edge((x0,y0),(x1,y1),…)` → grid matrix with index-based
directions). Defensive: labels are brace-wrapped (commas), empty cells get an
invisible `{}` node, self-loops/dir-only edges are skipped — so pathological
"automaton" diagrams compile (if degenerately) instead of breaking. Preamble
gets `\usepackage{tikz-cd}`.

NOTE: the more general/principled path (not taken) is to shim **cetz**
(fletcher's rendering backend) directly, which would also cover raw cetz
canvases — but it yields lossy coordinate-TikZ rather than semantic tikz-cd and
would require reimplementing cetz's geometry (fletcher calls cetz expecting real
behaviour, so a marker-shim breaks its layout).

### Superseded: image mode (`--unsupported=image`)
~30 diagram sites. Finish the plan's image mode: render the diagram subtree via
`typst` to SVG/PDF and emit `\includegraphics`. Doubles as the general fallback
for context-bound subtrees.

## Cross-cutting gaps
- **Numbered equations**: `#set math.equation(numbering:"(1)")` — currently
  unnumbered `\[…\]`; should use `equation`/`align` for numbered, referenceable
  math. (Cross-refs to equations therefore resolve only approximately.)
- ~~`\label` emission~~ — done (item 4).
- ~~Page geometry~~ — done: `extract::get_page_size` reads
  `#set page(width/height)` → `geometry` `paperwidth`/`paperheight`. (The bug
  was that `PageElem::width`'s default is `Smart::Custom(A4)`, so a plain
  `chain.get` returned A4; fixed by checking `chain.has(...)` first.)
