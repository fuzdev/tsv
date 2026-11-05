# Switch discriminant, comment in a stripped operand pair

The `switch` twin of
[if/condition_stripped_operand_paren_comment](../../if/condition_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of the header's
expression stays in the header after the expression, which breaks open around it, as the bare
spelling does. tsv prints that form in one pass; prettier's first pass splits the
expression's binary chain further (`prettier_intermediate_shell`) and its second pass undoes
that, the same fixed point.
An own-line comment in a `switch` with cases is the exception: prettier moves it into the body
for the bare spelling too, the switch-discriminant relocation in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation);
tsv keeps it in the header for both spellings.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence).
