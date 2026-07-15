# expanding_no_ws_prettier_divergence

A `<slot>` whose only child is an expanding block (`{#if}` / `{#each}` / `{#key}`), authored with no
whitespace at either content boundary.

The block forces the slot multiline, and it lays out **block-style** — both tags intact, content on
its own indented line — exactly like a regular element in the same shape
([elements/inline_with_if_block](../../../elements/inline_with_if_block_prettier_divergence/)).
`<slot>` runs the same layout analysis as every other element, so it cannot drift from that.

The hugged boundary is render-free under Svelte 5 (start/end-of-content whitespace is removed at
compile), so it carries no signal and must not select the layout. Prettier reads it as an instruction
to keep the content glued to the tags and, having nowhere to put the content, dangles **both**
delimiters — `<slot⏎\t>{#if cond}text{/if}</slot⏎>`. `prettier_variant_dangle` is that form, which
prettier keeps stable and tsv normalizes to `input`.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
