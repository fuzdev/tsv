# spaces_prettier_divergence

The if-family **maximal space injection**: a space at every position tsv collapses —
tag-internal (`{#if  a  }`, `{:else  if  b  }`, `<div  >`, `{/if }`), block-element
content boundaries (`<div  > text </div  >`), and **section boundaries**
(`{#if  a  } t {/if }`) — across the family's shapes: div body, text body,
`{:else if}` forms, a space-only (empty) body, and a space-only `{:else}` branch.
tsv normalizes every case to the glued `input.svelte`: all of that whitespace is
render-free (the compiler trims tag-internal and every fragment edge), so none of it
survives inline or selects the layout.

Prettier collapses the tag-internal and div-content spaces but never a **section
boundary** space — it expands those constructs to multiline instead (a render-free
space selecting the whole construct's form), turns the space-only empty body into the
blank-line block form, and keeps the **last block in a file** spaced as authored (its
position quirk).

The shapes also cover a **comment at the boundary** (`{#if  a  } <!-- c --> t3 {/if }`):
the space before the comment is a fragment edge (trimmed); the space between the comment
and the content is inter-sibling (kept — under `preserveComments` it renders, so trimming
it would not be render-safe).

- `unformatted_ours_spaces.svelte` — the maximal spaced authorings; tsv normalizes all
  of them to `input.svelte`. Prettier does not — hence `unformatted_ours_*`.
- `unformatted_ours_asymmetric.svelte` — **one-sided** and **tab** boundary runs
  (leading-only, trailing-only, mixed per-section, `\t` runs); tsv normalizes them all to
  the same `input.svelte`, so the trim cannot be a symmetric-only or space-only rule.
- `divergent_variant_half_hugged.svelte` — prettier's output on the asymmetric authoring:
  it preserves each authored glued side while breaking each spaced side (half-hugged
  forms), stable as such; tsv rewrites it to a third stable form (hug is all-or-nothing).
- `divergent_variant_expanded.svelte` — prettier's output on that authoring (first five
  blocks expanded, the file-last block still spaced). Prettier keeps it stable; tsv
  rewrites it to a third stable form.
- `variant_expanded_last_glued.svelte` — that third form, pinned: the expanded blocks
  stay multiline (newline-authored), the last block's boundary spaces glue. Dual-stable
  (both formatters keep it), so the fixture's three stable forms are all on disk.

## Reason

Svelte-mirror whitespace: in inline layout, every whitespace character tsv keeps is one
the compiler keeps — a space belongs *between* sibling nodes (render-significant), never
inside a tag or a section boundary (render-free). See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/boundary_space_trim](../../boundary_space_trim_prettier_divergence/) — the
  space-only boundary trim across all block families (its div case is the same kitchen
  sink for `{:else}`)
- [blocks/if/last_block](../last_block_prettier_divergence/) — prettier's last-block
  position quirk in isolation
- [blocks/empty_branch_collapse](../../empty_branch_collapse_prettier_divergence/) — the
  render-free principle for empty branches in block form
