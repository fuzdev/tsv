# sole_param_hug_authored_expansion_prettier_divergence

A sole parameter whose object-type annotation the author wrote **expanded**, at a width where
the flat form fits. tsv keeps the expansion; prettier collapses it.

- **tsv**: `function fn(a: {⏎\taaa: Aaaa;⏎}): void {}` — the authored break survives
- **Prettier**: `function fn(a: { aaa: Aaaa }): void {}`

The divergence is one of **normalization**, not of content: both forms are idempotent under
both formatters, the ASTs are identical, and nothing is dropped. It covers exactly the
positions where the sole parameter **hugs** — a value signature (function, `declare` function,
arrow, class method), a type-member signature (method, call, construct) and a function /
constructor type — pinned here across all of them, because one arm
(`build_hugged_literal_param_doc`) prints the annotation for all three parameter builders.

A **two-parameter** list stays here as the control outside it: no hug, so both formatters keep
the expansion.

An object **pattern** sits on both sides and takes two cells. The pattern's *own* authored break
is outside the divergence — prettier's `printObject` reads `hasNewLineAfterOpeningBrace` off the
first *property*, which an `ObjectPattern` parameter never satisfies here, so `{ aaa }: T` stays
flat in both formatters. But an object **type** annotating that pattern is squarely inside it,
and the break tsv keeps propagates into the pattern beside it: `({⏎\taaa⏎}: {⏎\taaa: Aaaa;⏎})`
where prettier prints `({ aaa }: { aaa: Aaaa })`. The pattern expanding is not an extra tsv
choice — it is what prettier itself prints at every position where it *keeps* the group (add a
second parameter and prettier emits exactly that shape, as the `two` cell shows); it follows
here only because the hug drops the pattern's group too, so the annotation's break is the
enclosing group's.

## Reason

Prettier's collapse is a **side effect, not a rule**. `printObject` (`print/object.js`) ends in
`group(content, { shouldBreak })`, where `shouldBreak` is the author's newline after `{`
(`objectWrap: "preserve"`, prettier's own default). For the sole parameter of a hugging
signature it returns the bare `content` instead — "so that the object breaks before the return
type" — and the `shouldBreak` the authored newline lived on goes with the group it was attached
to. So prettier honors `objectWrap: "preserve"` at every object-type position **except** this
one, where the option is silently lost.

tsv prints the same group-less content in the hug (that is what makes the object break before a
breakable return type — see
[sole_param_object_return_break_long](../sole_param_object_return_break_long/)) but keeps the
forced break, so authored expansion is preserved uniformly at every position. Authored
expansion is authoring signal; discarding it in one position while honoring it in every other
is the less defensible of the two shapes.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Hugged sole-parameter object type.
