# block_body_drop_nested_siblings_prettier_divergence

The uniform body-drop shown **embedded in realistic nested context with inline
elements around it** — assurance that it reads correctly in real templates, not just
in isolation.

- **Breadcrumb shape** — an `{#each}` whose breakable `<a>` body follows an inline
  `<a>Home</a>` sibling (the exact shape that was 2-pass non-idempotent in
  `fuz_ui/src/lib/Breadcrumb.svelte` before the breakable-hug path was removed). tsv
  **dangles the `</a>` closing `>`** onto its own line (axis-3 sibling-`>` dangle) and
  drops the body to its own line.
- **Nested if/else** — both `{#if}`/`{:else}` branch bodies drop, two inline `<span>`
  wrappers deep, with a `<span class="label">` sibling alongside. (This block is the
  sole content of its `<span class="value">` wrapper, so the wrapper's *opening* `>`
  already dangles — the same `>`-before-`{#…}` rule from the enclosing-element side.)

tsv dangles the sibling `>` and drops the body uniformly; prettier keeps the
`</a>{#each}` boundary hugged with the body on its own line (`output_prettier.svelte`),
and on the compact one-liner hugs the `}` and breaks the element internally
(`prettier_variant_inline.svelte`). Both are stable under their own formatter; tsv
normalizes prettier's hug and the compact one-liner (`unformatted_ours_compact.svelte`)
back to `input.svelte` in one pass.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads, body
shapes, and contexts (one-pass `conditional_group`, no breakable special-case), which
keeps the layout idempotent — including the breadcrumb shape where the breakable
element is not the first body node. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../../blocks/if/element_body_long_prettier_divergence/) — the same drop, standalone
- [elements/inline_component_else_body_long](../inline_component_else_body_long_prettier_divergence/) — breakable element in `{:else}`
- [if/element_body_deep_nested](../../blocks/if/element_body_deep_nested_prettier_divergence/) — the same drop, deeply nested (dropped element re-wraps)
