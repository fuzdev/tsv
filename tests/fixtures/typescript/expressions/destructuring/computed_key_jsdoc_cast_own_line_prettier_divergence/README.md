# computed_key_jsdoc_cast_own_line_prettier_divergence

An **own-line JSDoc cast comment in a destructuring pattern's computed key** — a plain
rename and a rename with a default:

```js
const { [
	/** @type {A} */
	(k1)]: v1 } = obj1;
```

A computed key's `[` gap cannot hang the cast's break — no operator line to end — so tsv
**reflows** it onto the `[`'s line, the fixed point the glued authoring already reaches
(`input.svelte`, `const { [/** @type {A} */ (k1)]: v1 } = obj1;`), matching what a plain
block comment in the same gap already does.

Prettier **agrees on the fixed point** (it preserves the cast in a JS file) but is
**non-idempotent** getting there: its first pass on the own-line authoring emits the
broken mid-line form inside an expanded pattern (`prettier_intermediate_own_line.svelte`)
and its second pass collapses that to `input.svelte`. tsv normalizes the
`unformatted_ours_own_line.svelte` and `unformatted_ours_break.svelte` authorings to
`input.svelte` in one pass.

## Reason

One fixed point per document, reached in one pass: the gap cannot hang the break, so tsv
reflows it fully — the form prettier itself converges to in two passes. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(the computed-key own-line cast entry); the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
