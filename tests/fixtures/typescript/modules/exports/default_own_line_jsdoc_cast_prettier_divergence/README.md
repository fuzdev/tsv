# default_own_line_jsdoc_cast_prettier_divergence

A JSDoc cast an `export default` value opens with, written on a line of its own
(`export default⏎/** @type {T} */⏎(x).b + q`). The keyword→value seam hangs the value under
the keyword, cast and all, as every value gap does for an own-line cast (an assignment's
`x =⏎/** @type {T} */⏎(y)` keeps the same shape in both formatters), so tsv lands on
`input.svelte`, and reaches it in one pass from the flush spelling
(`unformatted_ours_flush.svelte`):

```ts
export default
	/** @type {T} */
	(x).b + q;
```

Prettier keeps the comment on the keyword's line and the value flush below it
(`output_prettier.svelte`):

```ts
export default /** @type {T} */
(x).b + q;
```

tsv joins that form onto one line, which both formatters keep (`variant_inline.svelte`). A
lone cast operand keeps prettier's break in both formatters (`variant_keyword_line.svelte`).

The `export default` carve-out of the continuation-indent rule — prettier keeps the comment on
the keyword's line, tsv hangs an own-line comment indented. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
