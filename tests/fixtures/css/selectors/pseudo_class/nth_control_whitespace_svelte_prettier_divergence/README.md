# nth_control_whitespace_svelte_prettier_divergence

A C0 control character that Svelte's `parseCss` counts as whitespace — U+000B LINE
TABULATION (VT) or U+000C FORM FEED (FF) — at an An+B juncture: around the operator
(`2n<VT>+<VT>1`), around the `of` keyword (`2n + 1<VT>of<VT>.class1`), against the parens
(`(<VT>2n + 1<VT>)`), and inside a non-nth pseudo (`:is(2n<VT>+<VT>1)`). Svelte's
`REGEX_NTH_OF` is a JS regex and JS `\s` holds both characters, so every one of these
tokenizes exactly like its space-separated spelling, and tsv's scanner reads the same class
there.

## Svelte divergence

Only the `of` case, and only the standing
[nth_child_of](../nth_child_of_svelte_prettier_divergence/) nesting: Svelte folds the ` of `
into `Nth.value` (`"2n + 1 of "`) and reads `S` as sibling selectors, where tsv keeps
`Nth.value = "2n + 1"` with `S` under `Nth.selector`. The control characters add nothing to
the wire — they are the whitespace the fold already skips.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

The [combinator_control_whitespace](../../combinator_control_whitespace_prettier_divergence/)
rule one construct over. tsv normalizes each gap the way it normalizes every selector gap:
a single space around the operator and around `of`, nothing against a paren — the uniform
rule under which a tab, a newline run, and these control characters all collapse alike.
Prettier freezes the byte where it stands: `prettier_variant_vt` pins the forms it keeps
stable, and `unformatted_ours_vt_compact` the glued authoring both formatters move (tsv to
`input.svelte`, prettier to the variant). Prettier's FF handling is not even consistent with
its VT handling — `2n<FF>+<FF>1` comes back as `2n<FF> + 1` (the second character dropped)
and a leading `(<FF>2n` loses its FF where a leading `(<VT>2n` keeps it — so
`prettier_variant_ff` pins only the positions it keeps.

Per CSS Syntax 3 the two characters reach this result from different starting points: FF is
folded to a newline by [input preprocessing](https://www.w3.org/TR/css-syntax-3/#input-preprocessing),
so it *is* whitespace and tsv's collapse is spec-aligned; VT is a
[non-printable](https://www.w3.org/TR/css-syntax-3/#non-printable-code-point) code point the
spec does not count as whitespace, and tsv follows its drop-in oracle, which does. The
divergent set is exactly this ASCII pair — a non-ASCII space (`nth_nbsp_*`) is content both
formatters keep in place.

See [conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors).
