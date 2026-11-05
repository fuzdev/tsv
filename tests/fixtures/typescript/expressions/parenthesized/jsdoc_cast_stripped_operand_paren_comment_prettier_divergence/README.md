# JSDoc cast pair, comment in a stripped operand pair

The JSDoc-cast twin of
[value_shell_stripped_operand_paren_comment](../value_shell_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of a JSDoc cast's
inner (`/** @type {T} */ (a + (b // c⏎) // d⏎)`) stays inside the cast's pair after the
inner, ahead of a comment written after the pair, which follows on its own line. The cast's
pair always prints, so with nothing written after the stripped pair the comment stays inside
the cast's pair too (`/** @type {T} */ (⏎\ta + b // c⏎)`).

tsv: `/** @type {T} */ (⏎\ta + b // c⏎\t// d⏎)`, the two comments in the order written, each
on its own line — the form both formatters give the bare spelling
Prettier: the same form from the input itself, but from the stripped-pair spelling it puts
the two comments on one line, where they weld into one comment (`a + b // c // d`,
`audit_signature_shell`)

An own-line comment after the pair already takes a line of its own — that case is a control.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
