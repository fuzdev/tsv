# jsdoc_cast_own_line_prettier_divergence

An **own-line JSDoc cast comment in an `{expr}` tag** — at the root fragment and as
element content:

```svelte
{
	/** @type {A} */
	(aa)
}
```

The tag hugs its braces, so it cannot hang the cast's break; tsv **reflows** it — the
comment joins the tag's line and the cast glues to it, the fixed point the glued
authoring already reaches (`input.svelte`, `{/** @type {A} */ (aa)}`).

Prettier **drops the cast comment and its parens outright** (`{aa}` —
`output_prettier.svelte`): comment loss plus a semantic change.

`unformatted_ours_own_line.svelte` is the own-line authoring;
`unformatted_ours_break.svelte` the mid-line one with the `(` on the next line. tsv
normalizes both to `input.svelte` in one pass; prettier normalizes neither (it deletes
the comment).

**Parser: a match.** Svelte parses these expressions with `preserveParens: true`, so
the cast's parens are a `ParenthesizedExpression` that takes the cast comment as its
`leadingComments`, and `remove_parens` then discards the pair with it; tsv runs the
cast's parens as the same discarded node, so on both sides the comment attaches
nowhere and survives in the root `comments` array. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Reason

User comments are valuable and shouldn't be silently removed; tsv preserves the cast and
reflows the one break it cannot hang. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head](../../../../../docs/conformance_prettier_svelte.md#svelte-own-line-jsdoc-cast-at-a-braced-head);
the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
