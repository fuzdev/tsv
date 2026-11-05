# `case` test, comment in a stripped operand pair

The `case`-test twin of
[discriminant_stripped_operand_paren_comment](../discriminant_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of a `case` test
stays after the test, ahead of a comment written after the pair, which follows on its own
line, and the `:` drops one level in — the head→`:` gap's own layout
([case_before_colon_line_comment](../case_before_colon_line_comment_prettier_divergence/)).
A pair around the whole test, nested pairs, an own-line `//` inside the pair and an own-line
`//` after it take the same place. A block alone after the pair stays after the comment, in the
order written, where prettier's fixed point from the stripped-pair spelling puts it ahead of the
`//` (`case a + b /* d */: // c`).

tsv: `case a + b // c`, then `// d` and the `:` one level in, the two comments in the order
written, each on its own line — the form tsv gives the bare spelling (`case a + b // c⏎// d⏎:`)
Prettier: moves the comments past the `:` — from the input the first trailing the label and
the second into the body (`case a + b: // c⏎// d`, `output_prettier`), from the
stripped-pair spelling both on the label's line, where they weld into one comment
(`case a + b: // c // d`, `audit_signature_shell`)

With nothing written after the pair, the comment trails the label in both formatters — that
case is a control.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence),
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation),
and [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
(the `:` one level in).
