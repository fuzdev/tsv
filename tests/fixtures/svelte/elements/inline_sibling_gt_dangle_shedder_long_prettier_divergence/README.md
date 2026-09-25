# inline_sibling_gt_dangle_shedder_long_prettier_divergence

The sibling-`>` dangle onto a block (`inline_sibling_gt_dangle`) when the inline element handing
its `>` on is itself too wide for its line. Its content drops to its own indented line and **so
does its closing tag**, and when the `>` dangles it dangles from the closing-tag line onto the
block head (`<b class="…">⏎\t{y}⏎</b⏎>{#if c}`): the element lays out block-style exactly like
an element that keeps its `>`, and only the `>` moves. Before a block that stays inline and fits
after it, the `>` hugs the block instead (`⏎</b>{#if c}t{/if}`), as it does for an element that
fits.

The element's block-style content is also what a both-boundary newline authoring looks like, so
its role must not depend on how that content was written: an element multiline only by authored
line breaks (`<b>⏎{y}⏎</b>`) dangles its `>` exactly like its width-broken twin. That is a cost
tsv accepts: prettier keeps `</b>{#if c}` there. Before an inline block the same authoring keeps
`</b>{#if c}t{/if}`, which both formatters print. An element multiline by **structure**, one
holding a block (`{#if x}a{/if}`), keeps its `>` (`</a>{#each …}`): the control, which prettier
prints identically.

The 100/101 boundary is the element's line up to its dangled `>` (`<b class="…">{y}</b`): at 100
it stays flat, at 101 it goes block-style. That holds before a multiline block, where the `>`
dangles on both sides of the boundary, and before an inline block, where it dangles at 100 and
hugs at 101. The rule holds for the four rendering block heads (`{#if}`, `{#each}`, `{#await}`,
`{#key}`) and a block with an `{:else}` branch, for text and an expression tag in the element,
for a component, for the tail of a glued run and an element behind a glued comment, in both the
width-broken and the authored-multiline form, and in a block parent and an inline parent.

- `unformatted_ours_compact.svelte` — every case on one line but for the authored line breaks
  its case names and each block's own body lines. tsv normalizes it to `input.svelte`; prettier
  to `prettier_variant_hugged.svelte`.
- `unformatted_ours_half_block.svelte` — each broken element's content on its own line with its
  closing tag hugging that line (`⏎\t{y}</b⏎>{#if c}`, `⏎\t{y}</b>{#if c}t{/if}`), half
  block-style, and each authored-multiline element keeping its `>` (`</b>{#if c}`). tsv
  normalizes it to `input.svelte`; prettier to `prettier_variant_half_block_dangle.svelte`.
- `prettier_variant_half_block_dangle.svelte` — prettier's form of the half block-style
  authoring: the same but for the inline-block case, where the `>` dangles
  (`⏎\t{y}</b⏎>{#if c}t{/if}`). Prettier-stable; tsv normalizes it to `input.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: the element's
  opening `>` dangles onto its content's line (`<b class="…"⏎\t>{y}</b⏎>{#if c}`).
  Prettier-stable; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: every block-style element as tsv
  prints it, but its `>` hugged to a multiline block's head (`</b>{#if c}`).

Every form renders the same: the `>` moves only inside an end tag, and the content-boundary
whitespace the forms differ in is trimmed at compile.

## Reason

Design choice. An inline element whose content goes multiline lays out block-style, both tags
intact on lines of their own, and handing its `>` to a block does not exempt it: a content line
with the closing tag hugged to it is neither the flat nor the block-style form. Which elements
dangle is decided by *cause*, because the element's block-style output is byte-for-byte the
authored-multiline form: an authored-multiline element that kept its `>` would reprint the
width-broken one as `</b>{#if c}` on the next pass, while structure is a cause the dangle's own
output never rewrites. See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
(Sibling `>` dangle) and
[§Moving a run: break-before, travel, and welded units](../../../../../docs/conformance_prettier_svelte.md#moving-a-run-break-before-travel-and-welded-units).

## Related

- [elements/inline_sibling_gt_dangle](../inline_sibling_gt_dangle_prettier_divergence/) — the dangle for an element that fits, all block heads
- [elements/inline_sibling_gt_dangle_inline_block_long](../inline_sibling_gt_dangle_inline_block_long/) — a wide element that stays flat while its `>` dangles
- [elements/inline_break_before_gt_dangle_shedder_long](../inline_break_before_gt_dangle_shedder_long_prettier_divergence/) — the same shedder before a glued element
