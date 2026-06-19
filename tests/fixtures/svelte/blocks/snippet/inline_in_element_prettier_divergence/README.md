# inline_in_element_prettier_divergence

A `{#snippet}` inside an inline element (`<A>…{#snippet n(a, b)}…{/snippet}…</A>`)
whose construct exceeds printWidth.

tsv keeps the snippet **params inline** and **expands the body** onto its own line —
the same "middle zone" layout the other block heads use: the head `{#snippet n(a, b)}`
fits on its own line, so it does not wrap; only the body drops down (body whitespace
inside an inline element is render-non-significant). Prettier instead **wraps the
params** (function-style) and keeps the body inline.

```svelte
<A
	>ttext{#snippet n(a, b)}
		x
	{/snippet}</A
>
```

Prettier:

```svelte
<A
	>ttext{#snippet n(
		a,
		b,
	)}x{/snippet}</A
>
```

Both forms are stable under their own formatter (tsv keeps tsv's, prettier keeps
prettier's), so `prettier_variant_params_wrap.svelte` records prettier's param-wrap
normalization (which tsv normalizes back to `input.svelte`). The empty-`()` cases
diverge only once the **whole construct** still overflows after the element boundary
wraps — at 102 chars (`<A` wraps too), tsv drops the body to its own line while
prettier keeps it hugged; at ≤101 the body stays inline in both (the element-boundary
wrap alone resolves the overflow, so there is nothing left to expand).

## Reason

tsv expands a wrapped block's body uniformly across all block heads, including
inside inline elements (render-safe — block-body boundary whitespace is
non-significant there), and keeps a head flat when it fits on its own line rather
than wrapping its arguments/params (the one-pass middle-zone layout). Snippet
params are not special-cased, so they stay inline + body-expanded like every other
block; prettier is the one that wraps. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [blocks/each/long_prettier_divergence](../../each/long_prettier_divergence/) — the
  block-head dangle + body-expand + middle-zone layout, standalone
- [blocks/if/inline_element_long_prettier_divergence](../../if/inline_element_long_prettier_divergence/)
  — the same body-expand inside an inline element, for `{#if}`
