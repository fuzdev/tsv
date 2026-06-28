# var_empty_fallback_degenerate_prettier_divergence

A `var()` whose fallback contains no real token — only commas/whitespace, e.g.
`var(--a,,)` — has a value-less fallback. Per CSS Syntax 3 the fallback's
whitespace is trimmed and a value-less `<declaration-value>?` is empty, so the
canonical form is `var(--a,)`.

**tsv** collapses the degenerate fallback to that canonical empty form
(`var(--a,)`), following the spec. **Prettier** preserves the degenerate comma,
normalizing only the spacing to `var(--a, ,)` — stable in one pass. So
`prettier_variant_collapse` is the form prettier keeps stable that tsv normalizes
to `input`, and `unformatted_ours_collapse` is the messy authoring tsv collapses
to `input` in one pass.

## Reason

**Spec compliance**: tsv follows CSS Syntax 3 and collapses the value-less fallback
to the canonical `var(--a,)`; prettier preserves the degenerate comma. Not a
real-world construct. The valid empty-fallback round-trip — where tsv and prettier
agree in one pass — is the regular fixture
[var_empty_fallback](../var_empty_fallback/). See
[conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values)
("var() value-less fallback").
