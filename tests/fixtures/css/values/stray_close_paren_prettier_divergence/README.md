# stray_close_paren_prettier_divergence

A value carrying a `)` that closes nothing (`x ) y`, `screen ) print`).

Per CSS Syntax 3 a value is consumed as component values whatever they are, and a `)`
with no open `(` is a bare `<)-token>` between two others (§5.4.7, "Consume a component
value"). tsv treats the whitespace around it as it treats every separator: the run
collapses to one space, and the rest of the value normalizes as usual
(`x  )  .50PX` → `x ) 0.5px`, a list after it takes its `, `) — in a declaration value,
with or without a comment, and in an `@import` prelude alike, wherever the value reader
runs. Only the padding *inside* a group strips — `( x )` → `(x)` — and a stray `)` opens
no group, so `(a: .50PX )  )  y` → `(a: 0.5px) ) y`. (An `@supports` prelude is read by
the condition reader, which keeps a prelude it cannot structure verbatim —
[supports_unbalanced_paren](../../at_rules/supports_unbalanced_paren_prettier_divergence/).)

Prettier's value parser throws on the unbalanced paren, and prettier emits the whole
value **verbatim** — the same freeze as
[line_comment_arg](../functions/line_comment_arg_prettier_divergence/), where the
loose tokenizer's line comment is what unbalances the parens. So every authoring of the
value is a prettier fixed point, double spaces and `.50PX` included. With nothing after
the `)` prettier throws outright instead —
[trailing_close_paren](../trailing_close_paren_prettier_divergence/).

No `output_prettier.*`: on tsv's canonical form (`input`) prettier agrees, so the
divergence lives only in `prettier_variant_spaces` — a form prettier keeps stable that tsv
normalizes to `input`.

See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values).
