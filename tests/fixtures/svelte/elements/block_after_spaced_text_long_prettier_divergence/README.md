# block_after_spaced_text_long_prettier_divergence

The width boundary of [block_after_spaced_text](../block_after_spaced_text_prettier_divergence/):
a block element after a first-child spaced text takes its own line at **every** width. At exactly
100 chars the hugged line would fit, and prettier keeps it (`prettier_variant_hugged.svelte`);
tsv still breaks before the block. At 101 the hugged line overflows: tsv breaks before the block,
which then collapses inline on its fresh line (89 chars), where prettier opens the block
block-style with its head left dangling on the text line (`prettier_variant_dangled.svelte`) —
tsv normalizes that form back to `input.svelte` too, the block's content reflowing onto its
fresh line. `unformatted_ours_one_line.svelte` authors both cases on one line; tsv normalizes it
to `input.svelte` in one pass and prettier lands on the dangled form.

## Reason

Design choice — see the sibling fixture's README: the block's own break separates it from the
text, so no width makes the hug the right layout, and a multiline unit's head never ends a
content line.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
