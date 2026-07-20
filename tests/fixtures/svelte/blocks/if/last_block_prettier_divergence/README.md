# last_block_prettier_divergence

Prettier's handling of a symmetric-spaced if-body (`{#if a} t1 {:else if b} t2 {/if}`) is
**position-dependent**: it expands the form to multiline — except for the **last block in a
file**, which stays inline as authored. A single block alone appears preserved, but only
because it is last — not by design.

tsv instead **glues** the space-only boundaries uniformly, regardless of position: a
space-only section boundary is render-free (the compiler trims every fragment edge), so it
neither survives inline nor selects the layout. `input.svelte` holds two identical glued
blocks — one canonical form wherever the block sits.

- `unformatted_ours_spaces.svelte` — both blocks spaced; tsv normalizes both to
  `input.svelte`. Prettier expands the first and keeps the last spaced (the quirk), so it
  does not normalize to input — hence `unformatted_ours_*`.
- `divergent_variant_last_inline.svelte` — prettier's output on that spaced authoring
  (first expanded, last still spaced). Prettier keeps it stable; tsv rewrites it to a third
  stable form.
- `variant_expanded_last_glued.svelte` — that third form, pinned: the expanded block stays
  multiline (newline-authored), the last block's boundary spaces glue. Dual-stable (both
  formatters keep it), so the fixture's three stable forms are all on disk.

## Reason

Svelte-mirror whitespace + position-independence: the same document formats the same way
wherever it appears in a file. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/boundary_space_trim](../../boundary_space_trim_prettier_divergence/) — the
  space-only boundary trim across all block families
