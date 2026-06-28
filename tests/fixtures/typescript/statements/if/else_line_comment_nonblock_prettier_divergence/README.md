# else_line_comment_nonblock_prettier_divergence

A line comment between the `else` keyword and a non-block body expression
(`} else // c1`, then the body on the next line).

**Prettier** keeps the comment after `else` but drops it onto its **own line**,
indented with the body. **tsv** keeps the comment trailing `else` on the **same
line** (the author's placement), with the body indented beneath.

A third form — the comment relocated *before* `else` (`} // c1`, then
`else expr`) — is also prettier-stable, documented as the dual-stable
`variant_comment_before_else.svelte`.

## Reason

Per Comment Position Philosophy: preserve user intent (the comment trailing the
`else` keyword) rather than forcing it onto its own line.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
