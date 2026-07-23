#let rule(..args) = {
  let pos = args.pos()
  if pos.len() == 0 { return [] }
  let conclusion = pos.last()
  let premises = pos.slice(0, pos.len() - 1)
  [#metadata((type: "curryst-rule", premises: premises, conclusion: conclusion)) <curryst-rule>]
}

#let prooftree(r) = {
  [#metadata((type: "curryst-prooftree", rule: r)) <curryst-prooftree>]
}

#let rule-set(..trees) = {
  [#metadata((type: "curryst-rule-set", trees: trees.pos())) <curryst-rule-set>]
}
