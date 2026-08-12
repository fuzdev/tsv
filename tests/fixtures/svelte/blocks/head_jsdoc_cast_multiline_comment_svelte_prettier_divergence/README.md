# head_jsdoc_cast_multiline_comment_svelte_prettier_divergence

A **multi-line JSDoc cast comment given its own line at a block head** — the multiline
arm of the braced-head cast reflow
([blocks/head_jsdoc_cast_own_line](../head_jsdoc_cast_own_line_svelte_prettier_divergence/)):

```svelte
{#if
	/**
	 * @type {A}
	 */
	(aa)}
```

The head cannot hang the cast's break, so tsv **reflows** it — the comment joins the
head's line and the `(` glues to the closing `*/`. The comment's own interior newlines
are verbatim and force the head open, so the reflowed fixed point keeps the block-head
shape a comment-broken head always takes (`}` dangling at base — see
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)):

```svelte
{#if /**
 * @type {A}
 */ (aa)
}
```

Prettier **drops the cast comment and its parens outright** (`{#if aa}` —
`output_prettier.svelte`). `unformatted_ours_own_line.svelte` is the own-line authoring;
tsv normalizes it to `input.svelte` in one pass.

**Parser (vs Svelte).** Svelte parses the head with `preserveParens: true`, then
`remove_parens` discards the wrapper and its `leadingComments`; tsv attaches the comment
to the inner expression (`expected_ours.json` vs `expected_svelte.json`). See
[conformance_svelte.md §Comment Attachment Differences](../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Reason

User comments are valuable and shouldn't be silently removed; tsv preserves the cast and
reflows the one break it cannot hang. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head](../../../../../docs/conformance_prettier_svelte.md#svelte-own-line-jsdoc-cast-at-a-braced-head).
