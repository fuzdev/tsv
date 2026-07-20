# boundary_space_trim_prettier_divergence

A **space-only section boundary** in an `{#await}` — `{#await promise} text1 {:then value} …` —
is **render-free**: Svelte trims every fragment edge at compile (`clean_nodes`), so the spaces
never reach the DOM. tsv removes them in inline layout: every space-only boundary glues
(`{#await promise}text1{:then value}…`), across the full form, the then-only form, and the
`then` shorthand. A **newline**-authored boundary keeps its meaning (the construct stays
multiline) — a space is not intent.

Prettier instead **preserves** the spaced `{#await}` inline, keeping spaces the compiler
deletes. (Unlike `{#if}`/`{#each}`/`{#key}`/`{#snippet}`, which prettier *expands* on the same
authoring — the split is a plugin artifact: the `AwaitBlock` printer is the one block case
without `breakParent`, so only await's boundary `line`s can collapse.)

- `prettier_variant_spaces.svelte` — every section body spaced; prettier keeps it stable, tsv
  normalizes it to `input.svelte` (all glued).

## Reason

Svelte-mirror whitespace: in inline layout, every whitespace character tsv keeps is one the
compiler keeps — a space belongs *between* sibling nodes (render-significant, collapses to one
space), never inside a section boundary (render-free). See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/boundary_space_trim](../../boundary_space_trim_prettier_divergence/) — the
  `{#if}`/`{#each}`/`{#key}`/`{#snippet}` counterpart (prettier expands those; tsv glues both
  the same)
- [await/empty_catch](../empty_catch_prettier_divergence/) — the kept empty `:catch` (its
  separators already glue)
