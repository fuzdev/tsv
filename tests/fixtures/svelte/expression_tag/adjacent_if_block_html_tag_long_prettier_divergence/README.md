# adjacent_if_block_html_tag_long_prettier_divergence

An `{#if}` block sitting immediately after an {@html} tag (`{@html expr}`) — a **breakable** preceding
sibling, so the sibling breaks first and the block head is held flat.

The block's body still overflows and wraps. tsv lays that wrapped body out block-style: the
body drops to its own indented line and `{/if}` drops to its own line, head intact.
Prettier keeps the body **welded** to the head and the close
(`{#if …}, updated {fn(⏎…⏎)}{/if}`), which forces the inner call's arguments to break just
to make the welded line fit — see `prettier_variant_welded.svelte`, the form prettier keeps
stable and tsv converges to `input.svelte`.

The body boundary is render-free under Svelte 5, so it cannot select the layout. Whether the
head is *allowed* to wrap is a property of the head, not of the body: the body-expand
decision is made by width alone, so this block lays out the same as it would with no
preceding sibling at all.

Welding is not merely the worse layout, it is **unstable**. The welded body renders at the
column where the over-width head ended, so its fill breaks at *every* separator; formatting
that output again reads the newline it just injected as authored leading whitespace, takes
the multiline path, and settles on a different form — `format(format(x)) != format(x)`, an
F1 break. Expanding the body is what makes the emitted form its own fixed point.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)
and [§Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).
