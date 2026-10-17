# else_line_comment_nonblock_body_break_prettier_divergence

A line comment between the `else` keyword and a non-block body whose layout
breaks (`else // c`, then a callback call whose block body spans lines).

**Prettier** drops the comment onto its **own line**, indented with the body.
**tsv** keeps the comment trailing `else` on the **same line** (the author's
placement), with the body indented beneath — the same rule as a flat body
([else_line_comment_nonblock_prettier_divergence](../else_line_comment_nonblock_prettier_divergence/)),
held when the body breaks.

Prettier's own-line form is dual-stable: tsv keeps it verbatim too, so the two
authorings hold their own forms. Pinned as `variant_own_line.svelte`.

## Reason

Per Comment Position Philosophy: preserve user intent (the comment trailing the
`else` keyword) rather than forcing it onto its own line.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
