# With object, comment in a stripped operand pair

The `with` twin of
[if/condition_stripped_operand_paren_comment](../../statements/if/condition_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of the header's
expression stays in the header after the expression, which breaks open around it, as the bare
spelling does. tsv prints that form in one pass; prettier's first pass splits the
expression's binary chain further (`prettier_intermediate_shell`) and its second pass undoes
that, the same fixed point.

See
[conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence).
