# adjacent_if_block_long_prettier_divergence

An expression tag glued to an `{#if}` block (`{fn(…)}{#if …}…{/if}`, no whitespace
between) whose construct exceeds printWidth. The `{@html}` / `{@render}` forms are
[adjacent_if_block_html_tag_long](../adjacent_if_block_html_tag_long_prettier_divergence/)
and
[adjacent_if_block_render_tag_long](../adjacent_if_block_render_tag_long_prettier_divergence/).

**tsv** expands the block body onto its own line once the construct overflows — the
same block-expand it applies everywhere else (see
[if/inline_element_long](../../blocks/if/inline_element_long_prettier_divergence/)).
Content that fits stays inline (the 100-char case).

**Prettier** keeps the body hugging the head past printWidth and instead breaks
*inside* the content — splitting the text run mid-fill (`…},⏎\tupdated`) or breaking
the inner call's arguments (`{fn(⏎\t\tbbbbbbbbb⏎\t)}`). That form is stable for
prettier (`prettier_variant_compact`), and tsv normalizes it — and the compact
authoring (`unformatted_ours_compact`) — to `input.svelte` in one pass.

## Reason

tsv treats printWidth as a hard limit and prefers dropping the body to its own line
over breaking the content that sits on an already-over-width line.

Beyond layout taste, prettier's form is **not a fixed point for tsv's model**: hugging
the body means it renders at the column where the over-width head ended, so the
content fill breaks at *every* separator. Re-formatting that output reads the injected
newline as authored leading whitespace and settles on a different form — an F1
(idempotency) break. Expanding the body is what makes the emitted form its own fixed
point, whatever the authoring.

See [conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).
