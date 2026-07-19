# boundary_space_trim_prettier_divergence

A **space-only section boundary** in a block — `{#if cond} text1 {/if}` — is **render-free**:
Svelte trims every fragment edge at compile (`clean_nodes`), so the space never reaches the
DOM. tsv therefore removes it in inline layout: every space-only boundary glues, uniformly
across `{#if}` / `{:else}` / `{#each}` / `{#key}` / `{#snippet}` (`{#if cond}text1{/if}`).
A **newline**-authored boundary keeps its meaning (the construct stays multiline) — a space
is not intent.

Prettier instead **expands** the symmetric-spaced form to multiline — a render-free space
selecting the whole construct's layout (with a quirk: the last block in a file stays inline
as authored). tsv glues regardless of position.

- `unformatted_ours_spaces.svelte` — every section spaced; tsv normalizes it to `input.svelte`
  (all glued). Prettier expands it instead, so it does not normalize to input — hence
  `unformatted_ours_*`. The div case is the kitchen sink — a space at **every** collapsible
  position at once (`{#if  cond  } <div  > text1 </div  > {:else  } …`): tag-internal and
  block-element content-boundary spaces collapse under both formatters; the section-boundary
  spaces only under tsv.
- `variant_expanded.svelte` — prettier's output on that spaced authoring (every block
  multiline). Both formatters keep it stable (the boundaries are newline-authored), distinct
  from input — a dual-stable form.

## Reason

Svelte-mirror whitespace: in inline layout, every whitespace character tsv keeps is one the
compiler keeps — a space belongs *between* sibling nodes (render-significant, collapses to one
space), never inside a section boundary (render-free). See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/await/boundary_space_trim](../await/boundary_space_trim_prettier_divergence/) — the
  `{#await}` counterpart (prettier *preserves* its spaced form instead of expanding — a plugin
  artifact; tsv glues both the same)
- [blocks/empty_branch_collapse](../empty_branch_collapse_prettier_divergence/) — the same
  render-free principle applied to empty branches in block form
