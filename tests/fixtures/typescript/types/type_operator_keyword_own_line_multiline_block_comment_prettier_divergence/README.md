# type_operator_keyword_own_line_multiline_block_comment_prettier_divergence

An **own-line** multiline block comment in a prefix type operator's
(`keyof`/`typeof`/`readonly`) keyword→operand gap hangs the operand on its own
line, the comment kept where the author wrote it (`keyof⏎/* … */⏎B`).

- **tsv** keeps the comment on its own line after the operator and hangs the
  operand one level under it (`type K = keyof⏎\t/* … */⏎\tB`) — the shared
  keyword→value layout.
- **Prettier** pulls the comment *up* onto the operator line and leaves the
  operand flush at the operator's level (`type K = keyof /* … */⏎B`).

This is the **multiline** sibling of the single-line
[type_operator_keyword_own_line_block_comment](../type_operator_keyword_own_line_block_comment_prettier_divergence/):
a *single-line* block (any authoring) collapses inline (`keyof /* c */ B`), and a
**glued** multiline block also collapses inline (operand on the comment's closing
line, matching prettier — see
[keyword_value_glued_multiline_block_comment](../keyword_value_glued_multiline_block_comment/)).
Only a **line** comment
([type_operator_keyword_line_comment](../type_operator_keyword_line_comment_prettier_divergence/))
or an **own-line multiline** block (this fixture) still hangs the operand — and
there tsv keeps the comment where the author wrote it rather than relocating it,
per [Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
Covers `keyof`, `typeof` (a `TypeQuery` node), and `readonly`.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
