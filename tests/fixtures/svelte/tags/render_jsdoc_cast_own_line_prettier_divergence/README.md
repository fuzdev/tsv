# render_jsdoc_cast_own_line_prettier_divergence

An **own-line JSDoc cast comment at `{@render}`**, where the cast is the call's
**callee** (`{@render⏎\t/** @type {A} */⏎\t(fn)()}` — a render tag's expression must be
a call, so the cast sits on the value's left spine rather than at its root). The
prefixed tag cannot hang the cast's break, so tsv **reflows** it onto the tag's line —
`input.svelte` (`{@render /** @type {A} */ (fn)()}`), the fixed point the glued
authoring already reaches.

Because the cast paren is not the expression's root, prettier's template strip misses
it: prettier **keeps** the comment here (unlike every comment-dropping head in this
family) and shares tsv's glued fixed point — but it also holds the mid-line broken form
stable (`{@render /** @type {A} */⏎(fn)()}`), a second fixed point tsv normalizes,
pinned as `prettier_variant_break.svelte`. The divergence is one of normalization.

`unformatted_ours_own_line.svelte` is the own-line authoring: tsv normalizes it to
`input.svelte` in one pass; prettier reflows it only onto the broken variant form.

## Reason

One fixed point per document: the tag cannot hang the break, so tsv reflows it fully
rather than holding a stranded mid-line form. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head](../../../../../docs/conformance_prettier_svelte.md#svelte-own-line-jsdoc-cast-at-a-braced-head);
the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
