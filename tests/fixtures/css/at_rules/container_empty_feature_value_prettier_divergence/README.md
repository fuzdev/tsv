# container_empty_feature_value_prettier_divergence

A `@container` feature query with an **empty value** (`(a:)` — a colon with no
value, malformed but parseable). tsv normalizes the post-colon spacing to a single
space (`(a: )`), the same spec-compliant normalization it applies to every other
`@container` feature spacing (see
[container_spacing](../container_spacing_prettier_divergence/)). Prettier leaves
`@container` preludes **verbatim**, so it keeps both `(a:)` and `(a: )` as-is —
each is a prettier fixed point.

tsv: `@container (a:)` → `@container (a: )` (normalized)
Prettier: `@container (a:)` → `@container (a:)` (verbatim)

Both formatters are idempotent on the spaced `input.svelte`; they diverge only on
the compact form, which tsv rewrites and prettier keeps
(`prettier_variant_compact.svelte`).

## Reason

This is the empty-value facet of the sanctioned `@container` spacing divergence.
Before the fix that added it, the empty-value form was **non-idempotent**: `(a:)`
gained a space (`(a: )`) while `(a: )` had that space stripped back before the `)`
(`(a:)`), oscillating every pass. tsv now keeps the post-value-colon space before
`)`, so `(a: )` is a stable fixed point — matching how tsv normalizes the non-empty
case (`(min-width:100px)` → `(min-width: 100px)`).

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules).

## Fixture Structure

- `input.svelte` — the normalized, spaced form (stable for both tsv and prettier)
- `prettier_variant_compact.svelte` — the compact form prettier keeps stable but tsv normalizes to `input`
