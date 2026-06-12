# var_empty_fallback_degenerate_prettier_divergence

A `var()` whose fallback contains no real token — only commas/whitespace, e.g.
`var(--a,,)` — collapses to the canonical empty-fallback form `var(--a,)`.
Per css-syntax-3 the fallback's whitespace is trimmed and a value-less
`<declaration-value>?` is empty, so every value-less form is the same.

Both formatters reach the same stable output (`var(--a,)`); the difference is
only normalization speed:

tsv: `var(--a,,)` → `var(--a,)` (1 pass, idempotent)
Prettier: `var(--a,,)` → `var(--a, )` (pass 1) → `var(--a,)` (pass 2)

Prettier is **non-idempotent** here — its first pass leaves a stray space
(`var(--a, )`) that a second pass removes. tsv normalizes directly to the
stable form. Not a real-world construct; this pins prettier's intermediate
form so the fixture audit doesn't flag it as novel.

## Reason

tsv normalizes consistently. Prettier's intermediate form is a stable quirk —
it takes two passes to converge. See the valid empty-fallback round-trip in
[var_empty_fallback](../var_empty_fallback/).
