# member_target_value_chain_long_prettier_divergence

An assignment whose **target** is a member chain, and whose **value** is a member chain too long
for its own line.

tsv: breaks the value's chain at its last lookup, holding print width
Prettier: prints the value's chain intact and overflows

| Case                                | tsv           | Prettier      |
| ----------------------------------- | ------------- | ------------- |
| `zz.yy = <value chain>` at 100      | intact        | intact        |
| `zz.yy = <value chain>` at 101      | breaks at the last lookup | intact, overflows |
| `zzyy = <value chain>` at 101       | breaks at the last lookup | breaks at the last lookup |

## Reason

Print width, and the scope of the target rule. `printMemberExpression`'s `shouldInline`
(member.js) inlines every lookup whose `firstNonMemberParent` is an assignment with a
non-`Identifier` `left` — the rule that makes an assignment **target** one unbreakable unit
(`assignment/member_target_long/`). Prettier reaches it with `path.findAncestor`, which is
position-blind: it walks up through every member ancestor without asking which side of the `=`
the chain sits on, so a value chain answers the same way purely because the *target* happens not
to be a plain identifier. The control row is the tell — the identical value chain keeps its break
points as soon as the target is `zzyy` instead of `zz.yy`.

tsv adopts the rule and not the walk. Not breaking the thing being assigned *to* is a real
argument; not breaking the thing being assigned is the same expression prettier breaks one
character of target-shape away, so the overflow buys nothing and costs the print-width limit.

What is declined is this clause's walk, not the value position: `shouldInline`'s **call-object**
clause names an assignment's value (and a declarator's initializer) outright and identity-checks
the parent, and tsv implements it — so a value chain whose last lookup hangs off a call *with
arguments* does lose that break point. The chains here are call-free, which is why they stay
breakable.

## Related

- `assignment/member_target_long/` — the target half of the same `shouldInline` clause, where tsv matches prettier.
- `assignment/breakable_lhs_template_rhs/` — the `chooseLayout` gate the target rule feeds (`canBreakLeftDoc`).
- `member/call_base_lone_tail_long/` — the `shouldInline` clause that DOES reach a value chain, where tsv matches prettier.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Assignment target member chains) and [§Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy).
