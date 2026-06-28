# compound_args_indent_long_prettier_divergence

When a pseudo-class's argument list breaks, prettier indents it an extra level if
the enclosing compound has more than two simple selectors (its flat
`selector-selector` node count exceeds 2). tsv indents broken pseudo args one
level relative to the selector, uniformly.

tsv: `.a.b:is(` args one level in, `)` aligned with the selector
Prettier: `.a.b:is(` args two levels in, `)` aligned with the rule body

## Reason

Design choice. Prettier's extra indent exists to align the continuation lines of a
combinator-broken complex selector; for a single compound (no combinator) there is
no continuation to align, so the extra level just nests the pseudo args deeper than
the rule body they belong to. tsv keys the indent on combinator presence instead of
a flat node count, so a single compound's pseudo args sit one level in and the `)`
aligns with the selector it closes. A combinator-bearing selector still indents its
continuation (matching prettier) — see
[combinators/pseudo_args_long](../../combinators/pseudo_args_long/). See
[conformance_prettier.md §CSS: Selectors](../../../../../../docs/conformance_prettier.md#css-selectors).
