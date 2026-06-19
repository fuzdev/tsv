# block_body_drop_in_context_prettier_divergence

The uniform body-drop shown **embedded in realistic nested context with inline
elements around it** — assurance that it reads correctly in real templates, not just
in isolation.

- **Breadcrumb shape** — an `{#each}` whose breakable `<a>` body follows an inline
  `<a>Home</a>` sibling (the exact shape that was 2-pass non-idempotent in
  `fuz_ui/src/lib/Breadcrumb.svelte` before the breakable-hug path was removed). The
  `</a>{#each …}` sibling boundary stays hugged; the body drops to its own line.
- **Nested if/else** — both `{#if}`/`{:else}` branch bodies drop, two inline `<span>`
  wrappers deep, with a `<span class="label">` sibling alongside.

tsv drops the body uniformly; prettier hugs the `}` and breaks the element internally
(`prettier_variant_hug.svelte`). Both are stable under their own formatter; tsv
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
- [components/attrs_nested_long](../../components/attrs_nested_long_prettier_divergence/) — the same drop seven levels deep
