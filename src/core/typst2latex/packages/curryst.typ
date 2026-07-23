// Markers are detected by their metadata `type`, not by label, so no `<...>`
// label is attached (an attached label would otherwise surface as a spurious
// `\label{...}` in the output).
#let rule(..args) = {
  let pos = args.pos()
  if pos.len() == 0 { return [] }
  let conclusion = pos.last()
  let premises = pos.slice(0, pos.len() - 1)
  [#metadata((type: "curryst-rule", premises: premises, conclusion: conclusion))]
}

#let prooftree(r) = {
  [#metadata((type: "curryst-prooftree", rule: r))]
}

#let rule-set(..trees) = {
  [#metadata((type: "curryst-rule-set", trees: trees.pos()))]
}
