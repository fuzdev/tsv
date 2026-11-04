# decorated_expr_close_paren_comment_prettier_divergence

A line comment before the **close** paren of a bare parenthesized decorated class expression
at statement position — the closing-delimiter mirror of
[decorated_expr_open_paren_comment](../decorated_expr_open_paren_comment_prettier_divergence/).
The pair is broken open around the decorators, so the comment stays inside it, where the
author wrote it: an own-line `//` on its own line above the `)`, a `//` glued to the class
after it.

**Prettier** moves the comment past `);`. There a trailing line comment already sits, and the
two **merge** into one comment (`// c1 // c2`, `output_prettier.svelte`) — the second
comment survives only as the first one's text. Behind a trailing block comment it lands on
the next line instead, after the block, out of authored order (`); /* c6 */⏎// c5`).

The rule is the line comment's, as for every retained grouping shell: an own-line **block**
comment before the `)` cannot swallow what follows it, so it prints past `);` in both
formatters (unlike the open-paren mirror, which keeps a comment of either kind inside).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
