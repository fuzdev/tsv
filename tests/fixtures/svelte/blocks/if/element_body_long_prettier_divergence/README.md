# element_body_long_prettier_divergence

A `{#if}` whose head fits but whose **breakable element body** (a component with
attributes) exceeds printWidth when hugged on the head line. tsv **drops the body to
its own line** — uniformly with every other block body (text, expression, void,
element) — so the construct goes multiline and the element stays on one line at the
body indent.

Prettier instead **hugs** the body to the `}` and breaks the element *internally*
(its attributes wrap), recorded in `prettier_variant_hug.svelte`. Both forms are
stable under their own formatter; tsv normalizes prettier's hug (and the compact
one-liner in `unformatted_ours_compact.svelte`) back to `input.svelte`.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all block heads and
body shapes (one-pass `conditional_group`, no breakable special-case), which keeps
the layout idempotent and consistent. Prettier's hug-and-break-internally is a body
layout driven by authored boundary whitespace, not width. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/long](../long_prettier_divergence/) — the head-wrap + dangle + body-expand divergence, standalone
- [key/void_element_body_long](../../key/void_element_body_long_prettier_divergence/) — the same drop for a void element body
- [await/element_body_long](../../await/element_body_long_prettier_divergence/), [snippet/element_body_long](../../snippet/element_body_long_prettier_divergence/) — the same drop inside an inline component
