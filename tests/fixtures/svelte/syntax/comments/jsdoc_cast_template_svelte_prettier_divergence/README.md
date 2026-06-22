# JSDoc type cast in Svelte template / directive positions

A JSDoc type cast (`/** @type {T} */ (expr)`) in a Svelte **template** position —
an attribute value (`title={…}`), a block test (`{#if …}`), or a mustache tag
(`{…}`, `{@html …}`) — diverges from both canonical tools at once, so this is a
`_svelte_prettier_divergence`.

**Formatter (vs prettier).** prettier-plugin-svelte formats these expressions
through a path that **strips** the cast (see `output_prettier.svelte`: `{x}`,
`title={x}`, `{#if x}`, `{@html x}`). tsv **preserves** the parens everywhere —
they are semantically required (without them the assertion is dropped). This is
narrower than the `<script>` JS-vs-TS split: in a template prettier strips even a
plain (JS) component. See
[conformance_prettier.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier.md#jsdoc--paren-semantics).

**Parser (vs Svelte).** Svelte parses template expressions with
`preserveParens: true`, then `remove_parens` discards the wrapper **and its
`leadingComments`**, so the cast comment survives only in the root `comments`
array. tsv (no `ParenthesizedExpression` node) attaches it to the inner
expression (`expected_ours.json` vs `expected_svelte.json`). The comment is never
lost; only its attachment differs, and `ast_diff` confirms equivalence. This is
the JSDoc-cast case that the sibling
[template_expr_paren_comment_svelte_divergence](../template_expr_paren_comment_svelte_divergence/)
deliberately deferred (it uses precedence parens to isolate the parser
difference). See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md).

**`{@const}` is worse — a prettier bug.** `{@const y = /** @type {T} */ (z)}`
makes prettier-plugin-svelte emit **invalid** output `(z}` (it drops the `)`) and
then throw when re-parsing its own output. tsv preserves it correctly and
idempotently. Because prettier produces no valid, stable output there, it cannot
be pinned as an `output_prettier.*` oracle and is documented in prose only.
