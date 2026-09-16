# inline_if_sibling_fill_long_prettier_divergence

A `{#if}` preceded by an inline `<span>` sibling, whose own body is a breakable
`<span>`, nested in a `<small>` (fill context). When the hugged content exceeds
printWidth, tsv **dangles the `</span>` closing `>`** onto its own line so the `{#if}`
head starts fresh. The `>` moves only *inside* the closing tag (`</span⏎>`), injecting
no whitespace between `</span>` and `{#if}`, so it is render-safe.

Both sides of the boundary that fresh line then faces are pinned, one tree each, at the
exact 100/101 column:

- **100** — the whole block fits on the dangled line, so it **stays inline** there
  (`>{#if …}<span>…</span>{/if}`). This is the middle state of the three-way placement:
  dangle without expanding.
- **101** — the dangled line overflows too, so tsv additionally **drops the `{#if}` body
  to its own line**.

Prettier diverges at both widths: it keeps `</span>{#if}` hugged, and its layout is
driven by the body's authored form rather than by width — the body on its own line for
this input (`output_prettier.svelte`), the inner `<span>` broken internally on the
compact one-liner (`prettier_variant_inline.svelte`).

The plain-container twin of the 100 case is
[inline_sibling_gt_dangle_inline_block_long](../inline_sibling_gt_dangle_inline_block_long/),
where tsv and prettier **agree** — the same middle state is a divergence only where a
preceding inline sibling puts the block in a fill.

## Reason

tsv dangles the `>` token immediately preceding a block's `{#…}` (a preceding inline
sibling's closing `>` here) whenever the hugged line overflows, and — once the dangled
line overflows too — expands the body uniformly across all block heads and body shapes.
See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Related

- [if/element_body_long](../../blocks/if/element_body_long_prettier_divergence/) — the same drop, standalone
- [if/element_body_deep_nested](../../blocks/if/element_body_deep_nested_prettier_divergence/) — a `{#if}` body drop at deep nesting
