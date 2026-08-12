# computed_key_jsdoc_cast_own_line_prettier_divergence

An **own-line JSDoc cast comment in an object literal's computed key** — a property, a
method, and a `get` accessor:

```js
const o = {
	[
	/** @type {A} */
	(k1)]: 1
};
```

A computed key's `[` gap cannot hang the cast's break — there is no operator line to end,
the same shape as a Svelte braced head — so tsv **reflows** it: the comment joins the
`[`'s line and the cast glues to it, the fixed point the glued authoring already reaches
(`input.svelte`, `[/** @type {A} */ (k1)]: 1`). A plain (non-cast) block comment in the
same gap already reflows this way; the cast now matches it.

Prettier preserves the cast in a JS file and **agrees on the property's fixed point**,
but is **non-idempotent** and splits the family: from the own-line authoring its first
pass emits the broken mid-line form for every member
(`prettier_intermediate_to_variant_own_line.svelte`), and its second pass reflows the
**property** key to the glued form while holding the **method** and accessor keys broken
— a mixed stable form pinned as `prettier_variant_break.svelte`, which tsv normalizes
whole to `input.svelte`. `unformatted_ours_own_line.svelte` and
`unformatted_ours_break.svelte` are the own-line and mid-line authorings: tsv normalizes
both to `input.svelte` in one pass.

## Reason

One fixed point per document: the gap cannot hang the break, so tsv reflows it fully —
reaching in one pass the form prettier itself converges to in two. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(the computed-key own-line cast entry); the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
