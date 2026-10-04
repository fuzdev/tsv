# jsdoc_cast_own_line_prettier_divergence

An **own-line JSDoc cast comment in a braced attribute value** — a component prop, an
event-handler attribute, a spread (`{...}`), an `{@attach}`, a `style:` directive value
(Svelte models it as an expression tag), and `<svelte:element this={…}>`:

```svelte
<Comp1 prop={
	/** @type {A} */
	(aa)
} />
```

These value positions hug their braces, so none can hang the cast's break; tsv
**reflows** it — the comment joins the value's line and the cast glues to it, the fixed
point the glued authoring already reaches (`input.svelte`,
`<Comp1 prop={/** @type {A} */ (aa)} />`).

Prettier **drops the cast comment and its parens outright** at every one of these
positions (`prop={aa}` — `output_prettier.svelte`): comment loss plus a semantic change.

`unformatted_ours_own_line.svelte` is the own-line authoring;
`unformatted_ours_break.svelte` the mid-line one with the `(` on the next line. tsv
normalizes both to `input.svelte` in one pass; prettier normalizes neither (it deletes
the comment).

The expression-value **directives** (`on:` / `bind:` / `class:` / `use:` /
`transition:`) are deliberately **not** in this family: their block form gives the
comment a properly indented line of its own, so the authoring survives — see
[directives/on/jsdoc_cast_own_line](../../directives/on/jsdoc_cast_own_line_prettier_divergence/).

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
