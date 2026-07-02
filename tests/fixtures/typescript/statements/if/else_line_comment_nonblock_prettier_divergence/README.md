# else_line_comment_nonblock_prettier_divergence

A line comment between the `else` keyword and a non-block body expression
(`} else // c1`, then the body on the next line).

**Prettier** keeps the comment after `else` but drops it onto its **own line**,
indented with the body. **tsv** keeps the comment trailing `else` on the **same
line** (the author's placement), with the body indented beneath.

A third placement — the comment relocated *before* `else` (`} // c1`, then
`else expr`) — is also prettier-stable and dual-stable (both formatters keep it
verbatim), documented as `variant_comment_before_else.svelte`.

For the non-block-consequent case (`if (a)⏎expr; // c1⏎else`), that same
comment-before-`else` form is *not* dual-stable: tsv re-collapses `if (a)⏎expr;`
to `if (a) expr;`, reaching a third stable form distinct from both prettier's and
the input. That divergent-variant case is pinned by `divergent_variant_nonblock.svelte`.

## Reason

Per Comment Position Philosophy: preserve user intent (the comment trailing the
`else` keyword) rather than forcing it onto its own line.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
