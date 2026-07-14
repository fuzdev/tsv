# keyword_paren_line_comment_prettier_divergence

A **line** comment between a control-flow keyword and its condition `(`
(`if // c⏎(a)`) — the line-comment counterpart of the block-comment
`keyword_paren_comment_prettier_divergence`.

- **tsv**: keeps the comment trailing the keyword, with `(` broken to the next
  line so the `//` can't swallow it. Uniform across `if`/`while`/`for`/`switch`/`catch`.
- **prettier**: relocates the comment *inside* the condition parens
  (`if (⏎ // c⏎ a⏎)`), and for `for` past the header to before the body
  (`for (;;) // c⏎{`).

Emitting the comment inline (the previous behavior) let the `//` run to
end-of-line and swallow the `(` — non-idempotent content loss that failed to
reparse. Per comment placement policy, the authored position is preserved.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-relocation) §Comment relocation.
