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

The mechanism is positional, not a trim that half-fires: prettier-plugin-svelte's
`printChildren` hands a fragment's first or last child straight to the text printer, ahead of the
edge trims a text between two siblings gets, so the stray space appears only when the text is the
fragment's **last** child (as it is here) — a mid-fragment text after a block is trimmed clean by
`trimTextNodeLeft`. It is the same early return that hugs a block to a **first**-child text, the
same artifact seen from the text's other edge: a stable hug at the text's trailing edge
([block_after_spaced_text](../block_after_spaced_text_prettier_divergence/)), an unstable stray
space at its leading edge (this fixture). tsv's children builder takes the trim at every position
and emits **no** fold/group after the block (the block's break already supplies the line), so no
leading space survives — the divergence the `prev_is_block_el` branch in `handle_text_child`
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
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
