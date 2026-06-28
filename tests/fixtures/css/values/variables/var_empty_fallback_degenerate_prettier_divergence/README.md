# var_empty_fallback_degenerate_prettier_divergence

A `var()` whose fallback contains no real token — only commas/whitespace, e.g.
`var(--a,,)` — has a value-less fallback. Per CSS Syntax 3 the fallback's
whitespace is trimmed and a value-less `<declaration-value>?` is empty, so the
canonical form is `var(--a,)`.

The two formatters reach **different** stable forms:

tsv: `var(--a,,)` → `var(--a,)` (collapses to the canonical empty fallback, per spec)
Prettier: `var(--a,,)` → `var(--a, ,)` (preserves the degenerate comma, just
normalizing the spacing — stable in one pass)

So `prettier_variant_collapse` (`var(--a, ,)`) is a form prettier keeps stable but
tsv normalizes to `input` (`var(--a,)`); `unformatted_ours_collapse` (`var(--a,,)`)
is the messy authoring tsv collapses to `input` in one pass while prettier settles
on the degenerate `var(--a, ,)`.

## Reason

**Spec compliance**: tsv follows CSS Syntax 3 and collapses the value-less fallback
to the canonical `var(--a,)`; prettier 3.9 preserves the degenerate comma. Not a
real-world construct. The valid empty-fallback round-trip — where tsv and prettier
agree in one pass — is the regular fixture
[var_empty_fallback](../var_empty_fallback/). See
[conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values)
("var() value-less fallback").
