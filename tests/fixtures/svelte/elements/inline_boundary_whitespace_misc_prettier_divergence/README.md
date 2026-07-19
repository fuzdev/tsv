# inline_boundary_whitespace_misc_prettier_divergence

The rest of prettier's whitespace-sensitive set, beyond ordinary inline elements: a table
cell (`<td>`), an `<option>`, and a foreign svg child (`<text>`). tsv's Svelte-mirror
trim is uniform — the compiler deletes every fragment-edge run at compile
(`clean_nodes`), regardless of the element's CSS display or namespace — so fits-inline
boundary spaces trim on all three. Prettier instead consults its per-element
whitespace-sensitivity data and preserves the authored boundary space on each.

- `prettier_variant_spaces.svelte` — prettier's stable spaced forms; tsv normalizes
  every case to the glued `input.svelte`.

Block elements, components, and `svelte:element` need no pin here — prettier trims
those boundaries too (its sensitivity data marks them insignificant), so they agree
with tsv.

## Reason

Same class as
[inline_boundary_whitespace](../inline_boundary_whitespace_prettier_divergence/), pinned
separately because prettier's sensitivity classification for these elements could move
independently of ordinary inline elements. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

## Related

- [title_boundary_whitespace](../title_boundary_whitespace_prettier_divergence/) — the
  `<title>`-as-RegularElement instance of the same class
- [inline_empty_space_misc](../inline_empty_space_misc/) — whitespace-only collapse
  across the same odd inline-classified tags (no divergence there)
