# inline_component_else_body_long_prettier_divergence

A `{#if}` with a long text consequent and an inline-component (`<Comp>`) or HTML
inline (`<strong>`) **`{:else}` body**. When the construct exceeds printWidth, tsv
**drops the whole block** — consequent and `{:else}` body each onto their own line —
while prettier keeps it inline and breaks only the closing `>` of the element
(`prettier_variant_hug.svelte`).

This is the non-first-section breakable case: the breakable element sits in `{:else}`,
behind an atomic consequent. tsv's uniform body-expand drops it cleanly in one pass
(the earlier breakable-hug path over-wrapped the head here — a 2-pass
non-idempotency now removed). At ≤100 chars the construct fits and stays inline in
both formatters.

## Reason

tsv expands a wrapped/overflowing block's body uniformly across all heads, sections,
and body shapes (one-pass `conditional_group`, no breakable special-case). See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [if/element_body_long](../../blocks/if/element_body_long_prettier_divergence/) — the same drop with the breakable body in the consequent
- [if/long](../../blocks/if/long_prettier_divergence/) — head-wrap + dangle + body-expand for `{#if}`
