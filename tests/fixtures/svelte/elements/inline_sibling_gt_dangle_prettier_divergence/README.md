# inline_sibling_gt_dangle_prettier_divergence

An inline-element sibling immediately before an **expanding** block: tsv dangles the
element's closing `>` onto its own line so the block head starts fresh
(`</span⏎>{#if…}`). A short block that stays inline keeps the `>` hugged. Prettier keeps
the `>` hugged in both.

- **Dangle case** — `<span>text</span>` directly before a block whose body overflows:
  the `</span>` closing `>` drops to the block-head line. The rule is uniform across
  the **four rendering block heads** — `{#if}`, `{#each}`, `{#key}`, and `{#await}`.
- **`{#snippet}` carve-out** — a snippet glued on one side does not dangle the `>`: it
  is a declaration and takes its **own line**, so the glued boundary splits instead (the
  break is render-free — the snippet hoists). A snippet glued to content on BOTH sides
  keeps the author's line and stays in the dangle regime — its multiline form dangles the
  preceding `>` exactly like `{#if}`. See
  [blocks/snippet/own_line](../../blocks/snippet/own_line_prettier_divergence/).
- **Control** — the same `<span>` before a short `{#if cond}text{/if}` that stays
  inline: the `>` keeps hugging (no dangle), because the block never goes multiline.

`{#await}` doesn't force its parent multiline on its own (a lone block stays inline,
matching prettier), but a preceding sibling routes it through the same multiline layout
the others use, so the dangle resolves in one pass.

The dangle moves the `>` only *inside* the closing tag (`</span⏎>`), injecting no
whitespace between `</span>` and `{#if}`, so it parses to a byte-identical AST — it is
render-safe. Prettier never expands the block, so it keeps `</span>{#if…}` hugged
(`output_prettier.svelte`).

## Reason

The `>` token immediately preceding an expanding block's `{#…}` dangles onto the
block-head line — the closing `>` of a preceding inline sibling exactly as the opening
`>` of an enclosing inline element already does for a sole-content block. Gated on the
block actually rendering multiline (a short inline block keeps the `>` hugged). See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [elements/block_body_drop_nested_siblings](../block_body_drop_nested_siblings_prettier_divergence/) — the breadcrumb `</a>` dangle in realistic context
- [elements/inline_if_sibling_fill_long](../inline_if_sibling_fill_long_prettier_divergence/) — the `</span>` dangle in a fill
