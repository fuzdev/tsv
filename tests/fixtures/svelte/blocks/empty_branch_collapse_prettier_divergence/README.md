# empty_branch_collapse_prettier_divergence

An **empty branch** of `{#if}` / `{:else if}` / `{:else}`, or an empty `{#each}` body or `{:else}`
fallback, inside a **block-form** (multiline) construct: tsv gives the empty section its own line
like any other, so the marker and the close each sit on a line with the empty body's blank line
between them (`{:else}⏎⏎{/if}`).

tsv normalizes to that shape **whatever the author wrote at the branch boundary** — a branch
authored glued (`{:else}{/if}`) and one authored across lines reach the same fixed point. Prettier
instead **preserves** the authored boundary: it leaves a newline-authored branch as
`{:else}⏎⏎{/if}` (where tsv agrees, so `input.svelte` is prettier-stable) but keeps a **glued**
one glued.

That preservation is what tsv declines: an empty branch renders nothing, so the whitespace inside
it is **render-free** and must not select the layout — the same principle behind the
all-or-nothing body-expand decision (see
[§Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks) and
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)).

Covered: `{#if}` empty `:else`, `{#if}` empty consequent, `{:else if}` empty branch, `{#each}` empty
`:else` fallback, `{#each}` empty body. A branch that carries content is untouched.

`prettier_variant_glued.svelte` authors every empty branch glued to its neighbour; prettier keeps
that form stable, tsv normalizes it to `input.svelte`.

Note the contrast with the **inline** form: when the whole construct fits on one line the empty
branch collapses and its markers glue ([blocks/if/empty](../if/empty/) pins
`{#if a}<div>text</div>{:else}{/if}`, which both formatters produce). The separators are soft — they
vanish inline and become newlines once the construct goes block-form — so the two shapes are one
rule, not two.

## Reason

Render-free boundary whitespace must not select the layout, and an empty branch is entirely
whitespace, so tsv picks one canonical block-form shape instead of preserving the authored one. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/if/empty](../if/empty/) — the inline empty-branch form (not a divergence; both formatters glue)
- [blocks/await/empty_catch_multiline](../await/empty_catch_multiline_prettier_divergence/) — the same
  block-form shape for a kept empty `{:catch}`
