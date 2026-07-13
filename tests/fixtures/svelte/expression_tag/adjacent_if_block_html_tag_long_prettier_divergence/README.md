# adjacent_if_block_html_tag_long_prettier_divergence

An `{@html}` tag glued to an `{#if}` block (`{@html fn(…)}{#if …}…{/if}`, no whitespace
between) whose construct exceeds printWidth — the same class as
[adjacent_if_block_long](../adjacent_if_block_long_prettier_divergence/), which carries
the full explanation; this pins the tag kind.

**tsv** expands the block body onto its own line once the construct overflows.
**Prettier** keeps it hugging the head past printWidth and splits the text run mid-fill
instead — a form stable for prettier (`prettier_variant_compact`) that tsv normalizes,
like the compact authoring (`unformatted_ours_compact`), to `input.svelte` in one pass.

## Reason

tsv treats printWidth as a hard limit; and prettier's hugged form is not a fixed point
for tsv's model — the body renders at the over-width head's column, so its fill breaks
at every separator, and re-formatting settles elsewhere (an F1 break). See
[adjacent_if_block_long](../adjacent_if_block_long_prettier_divergence/) and
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).
