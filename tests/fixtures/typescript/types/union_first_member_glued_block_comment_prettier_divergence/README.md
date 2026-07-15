# union_first_member_glued_block_comment_prettier_divergence

A run of block comments leading a union's **first** member, after the leading `|` (a line
comment in that gap is what forces the multiline leading-pipe form).

The **run itself matches prettier**: a pair the author glued stays glued and the member breaks
below (`A`), and blocks the author put on their own lines keep them (`B`). That is prettier's
`printLeadingComment` — the separator after each comment is read from the source around *that*
comment, never from where the member starts — applied through tsv's one shared
leading-comment emitter.

## The divergence

Only the **encoding of the offset**. Both formatters offset the run and the member past the
`|` by prettier's per-member `align(2, …)` (`union-type.js`); prettier emits that 2-column
offset as `tab + 2 spaces` under `--use-tabs`, tsv rounds it up to one whole tab. At
`tabWidth = 2` the two are the same visual width. Not specific to a comment run — any union
member with a leading line comment shows it, as case A of
[union_intersection_parens_leading_line_comment](../union_intersection_parens_leading_line_comment_prettier_divergence/)
pins.

See [conformance_prettier.md §Tabs-Only Alignment](../../../../../docs/conformance_prettier.md#tabs-only-alignment)
and the [Tabs-Only Indentation Philosophy](../../../../../docs/conformance_prettier.md#tabs-only-indentation-philosophy).
