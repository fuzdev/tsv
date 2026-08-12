# html_jsdoc_cast_own_line_svelte_prettier_divergence

An **own-line JSDoc cast comment at `{@html}`** (`{@html⏎\t/** @type {A} */⏎\t(aa)}`).
The prefixed tag cannot hang the cast's break — the value starts right after `{@html `
— so tsv **reflows** it: the comment joins the tag's line and the cast glues to it, the
fixed point the glued authoring already reaches (`input.svelte`,
`{@html /** @type {A} */ (aa)}`).

Prettier **drops the cast comment and its parens outright** (`{@html aa}` —
`output_prettier.svelte`): comment loss plus a semantic change.

`unformatted_ours_own_line.svelte` is the own-line authoring;
`unformatted_ours_break.svelte` the mid-line one with the `(` on the next line. tsv
normalizes both to `input.svelte` in one pass; prettier normalizes neither (it deletes
the comment).

**Parser (vs Svelte).** Svelte parses these expressions with `preserveParens: true`,
then `remove_parens` discards the wrapper **and its `leadingComments`**, so the cast
comment survives only in the root `comments` array; tsv (no `ParenthesizedExpression`
node) attaches it to the inner expression (`expected_ours.json` vs
`expected_svelte.json`). The comment is never lost; only its attachment differs. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Reason

User comments are valuable and shouldn't be silently removed; tsv preserves the cast and
reflows the one break it cannot hang. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head](../../../../../docs/conformance_prettier_svelte.md#svelte-own-line-jsdoc-cast-at-a-braced-head);
the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
