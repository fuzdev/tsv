# empty_branch_collapse_prettier_divergence

An **empty branch** of `{#if}` / `{:else if}` / `{:else}` / `{#each}` / `{:else}` fallback
**collapses**: it emits no body and its markers **glue** (`{:else}{/if}`, `{#if cond}{:else}`,
`{#each items as item}{:else}`), rather than straddling a blank line.

tsv applies this **uniformly, whatever the author wrote at the branch boundary** — an empty branch
authored glued and one authored across lines reach the same fixed point. Prettier instead
**preserves** the authored boundary: it keeps the glued form glued (so `input.svelte` is
prettier-stable and tsv matches it exactly), but a branch authored across lines is left as
`{:else}⏎⏎{/if}` — marker, blank line, close.

That preservation is what tsv declines: an empty branch renders nothing, so the whitespace inside it
is **render-free** and must not select the layout — the same principle behind the all-or-nothing
body-expand decision (see [§Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks)
and [§Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)).
Collapsing is also what tsv already did for these constructs **inline**
([blocks/if/empty](../if/empty/) pins `{#if a}<div>text</div>{:else}{/if}`), so the block form now
agrees with the inline form instead of contradicting it.

Covered: `{#if}` empty `:else`, `{#if}` empty consequent, `{:else if}` empty branch, `{#each}` empty
`:else` fallback, `{#each}` empty body. A branch that carries content is untouched — it keeps its own
indented line and the close drops to its own line as usual.

`unformatted_ours_newline.svelte` authors every empty branch across lines; tsv collapses each to
`input.svelte`. Prettier leaves them as marker + blank + close, so it does not normalize to
`input.svelte` — hence `unformatted_ours_*`.

`prettier_variant_blank.svelte` is that preserved form, which prettier keeps stable and tsv
normalizes to `input.svelte` — the two formatters' endpoints for the newline authoring, pinned side
by side.

## Reason

Render-free boundary whitespace must not select the layout, and an empty branch is entirely
whitespace. One canonical collapsed form keeps the block form consistent with the inline form and
with `{#await}`'s empty-section collapse. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/if/empty](../if/empty/) — the inline empty-branch form (not a divergence; both formatters glue)
- [blocks/await/empty_catch_multiline](../await/empty_catch_multiline_prettier_divergence/) — the same
  empty-section collapse for a kept empty `{:catch}`
