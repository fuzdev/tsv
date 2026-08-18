# keyword_paren_line_comment_prettier_divergence

A **line** comment between a control-flow keyword and its condition `(`
(`if // c⏎(a)`) — the line-comment counterpart of the block-comment
`keyword_paren_comment_prettier_divergence`.

- **tsv**: keeps the comment trailing the keyword, with `(` broken to the next
  line so the `//` can't swallow it. Uniform across `if`/`while`/`for`/`switch`/`catch`.
- **prettier**: relocates the comment *inside* the condition parens
  (`if (⏎ // c⏎ a⏎)`), and for `for` past the header to before the body
  (`for (;;) // c⏎{`).

Emitting the comment inline let the `//` run to end-of-line and swallow the `(` —
non-idempotent content loss that failed to reparse. Per comment placement policy,
the authored position is preserved.

⚠️ The **flush** continuation here is the family's own answer, not an unreached
site of [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
All five statement headers agree on it (`for await` too, its extra gap pinned by
[for_await_keyword_line_comment](../../../statements/for/for_await_keyword_line_comment_prettier_divergence/)),
and what continues here is not a construct's tail but the whole rest of the
statement — condition, `{`, body and closing `}` — which the body's own indent
already positions. The expression-level keyword→operand gaps, where the tail *is*
an operand, do take the indent
([await_new_operand_line_comment](../../../expressions/await_new_operand_line_comment_prettier_divergence/)).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
