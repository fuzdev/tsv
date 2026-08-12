# init_binding_equals_own_line_block_comment_prettier_divergence

An **own-line** block comment in a for-init binding's `=` gap
(`for (let g⏎/* c */⏎= 0; …)`). A single-line block ends nothing, so the breaks
around it are unforced: tsv collapses them in one pass and keeps the comment
trailing the binding (`let g /* c */ = 0`, `input.svelte` — the inline authoring
both formatters hold stable, pinned as the plain
[init_binding_equals_comment](../init_binding_equals_comment/)). Prettier instead
**relocates** the comment across the `=` and hangs it leading the value, breaking
the header open (`variant_own_line.svelte`).

```
// tsv (collapse, comment trails)        // prettier (relocate past `=`, hang)
for (let g /* c */ = 0; g < 10; g++) {}  for (
                                         	let g =
                                         		/* c */
                                         		0;
                                         	g < 10;
                                         	g++
                                         ) {}
```

Prettier's landing is **dual-stable** — an own-line comment after `=` hangs under
tsv too — so it is pinned as `variant_own_line.svelte` rather than as prettier's
output from input (prettier holds `input.svelte` unchanged, so there is no
`output_prettier.svelte`). The divergence rides the own-line authoring
(`unformatted_ours_own_line.svelte`), which tsv normalizes to input in one pass.

The for-init face of the variable-declarator
[declarator_before_eq_own_line_block_comment](../../../declarations/variable/declarator_before_eq_own_line_block_comment_prettier_divergence/),
whose gap this one answers identically. The **line**-comment spelling of the same
gap (which does force a break, so the comment holds its own line under tsv) is
[init_binding_equals_line_comment](../init_binding_equals_line_comment_prettier_divergence/).

See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation
and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy.
