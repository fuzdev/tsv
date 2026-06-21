# space_after_block_prettier_divergence

A block element (`<div>`) directly followed by content text. tsv trims the boundary whitespace and
puts the text on its own line (the block already supplies the separating break), so the result is
the same in one pass regardless of how the source was authored. Prettier reaches the same fixed
point but is **non-idempotent** from the same-line authoring: fed `<div>block</div> text` on one
line it strands a **leading space** before the text (`>⏎ text after the block`), then trims it on
the next pass.

tsv: text on its own line, boundary trimmed — one pass from either authoring
Prettier: strands a leading space on the text line from the same-line authoring, converging only on
a second pass

This is prettier-plugin-svelte's `trimTextNodeLeft` boundary: when a block element precedes content
text with a same-line (space, no linebreak) boundary, prettier trims the text's leading whitespace
but its own block-child break still emits, leaving a stray space. tsv's children builder takes the
same trim but emits **no** fold/group after the block (the block's break already supplies the line),
so no leading space survives — the divergence the `prev_is_block_el` branch in `handle_text_child`
guards.

## Files

- `unformatted_ours_compact.svelte` — the same-line authoring; normalizes to `input.svelte` under
  tsv in one pass. Prettier does **not** normalize it to `input` (N6): its first pass is the
  stray-space form.
- `prettier_intermediate_compact.svelte` — prettier's unstable first-pass output of the compact
  form (the stray leading space); a second prettier pass converges to `input.svelte`.

## Reason

A comment's-worth of leading whitespace after a block element is not semantic, and the block's own
line break already separates it from the following text — so tsv trims it uniformly, in one pass,
rather than reproducing prettier's authoring-dependent stray space. See
[conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
