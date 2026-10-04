# JSDoc type cast in Svelte template / directive positions

A JSDoc type cast (`/** @type {T} */ (expr)`) in a Svelte **template** position —
an attribute value (`title={…}`), a block test (`{#if …}`), or a mustache tag
(`{…}`, `{@html …}`) — diverges from prettier; the parser side matches Svelte.

**Formatter (vs prettier).** prettier-plugin-svelte formats these expressions
through a path that **strips** the cast (see `output_prettier.svelte`: `{x}`,
`title={x}`, `{#if x}`, `{@html x}`). tsv **preserves** the parens everywhere —
they are semantically required (without them the assertion is dropped). This is
narrower than the `<script>` JS-vs-TS split: in a template prettier strips even a
plain (JS) component. See
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).

**Parser: a match.** Svelte parses template expressions with
`preserveParens: true`, then `remove_parens` discards the wrapper **and its
`leadingComments`**, so the cast comment survives only in the root `comments`
array. tsv runs a cast's parens as the same discarded node, so it attaches the
comment nowhere either. A **bare** grouping paren inside an expression keeps no
span in tsv, so a comment leading one is still attached to the inner expression —
the residual the sibling
[template_expr_paren_comment_svelte_divergence](../template_expr_paren_comment_svelte_divergence/)
pins. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

**`{@const}` is worse — a prettier bug.** `{@const y = /** @type {T} */ (z)}`
makes prettier-plugin-svelte emit **invalid** output `(z}` (it drops the `)`) and
then throw when re-parsing its own output. tsv preserves it correctly and
idempotently. Because prettier produces no valid, stable output there, it cannot
be pinned as an `output_prettier.*` oracle and is documented in prose only.
