# mixed_space_before_block_prettier_divergence

A one-word text followed by a spaced block element: the block takes its own line
(`text⏎<div>block</div>`). Prettier keeps the block hugged to the text when the text is the
fragment's first child (`prettier_variant_space.svelte`); tsv normalizes that spelling, the
compact authoring (`unformatted_ours_compact.svelte`) and the over-spaced one
(`unformatted_ours_spaces.svelte`) to `input.svelte`, where prettier lands the latter two on its
hugged form.

## Reason

Design choice — the one-word instance of
[block_after_spaced_text](../block_after_spaced_text_prettier_divergence/): a block sibling's own
break separates it from the text at every position of the text, so the space is spent on it.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
