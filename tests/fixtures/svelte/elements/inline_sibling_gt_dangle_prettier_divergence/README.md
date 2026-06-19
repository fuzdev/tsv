# inline_sibling_gt_dangle_prettier_divergence

An inline-element sibling immediately before an **expanding** block: tsv dangles the
element's closing `>` onto its own line so the block head starts fresh
(`</span⏎>{#if…}`). A short block that stays inline keeps the `>` hugged. Prettier keeps
the `>` hugged in both.

- **Dangle case** — `<span>text</span>` directly before a `{#if}` whose body overflows:
  the `</span>` closing `>` drops to the block-head line. `{#each}` and `{#key}` dangle
  identically (the rule is uniform across all three heads).
- **Control** — the same `<span>` before a short `{#if cond}text{/if}` that stays
  inline: the `>` keeps hugging (no dangle), because the block never goes multiline.

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

- [elements/block_body_drop_in_context](../block_body_drop_in_context_prettier_divergence/) — the breadcrumb `</a>` dangle in realistic context
- [elements/inline_if_sibling_fill_long](../inline_if_sibling_fill_long_prettier_divergence/) — the `</span>` dangle in a fill
