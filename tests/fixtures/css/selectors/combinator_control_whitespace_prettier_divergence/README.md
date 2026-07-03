# combinator_control_whitespace_prettier_divergence

A descendant combinator authored with a C0 control character that Svelte's
`parseCss` counts as whitespace — U+000B LINE TABULATION (VT) or U+000C FORM
FEED (FF). Svelte's selector reader uses JavaScript `\s` regexes, and JS `\s`
includes both, so `div<VT>p` and `div<FF>p` tokenize exactly like `div p`: one
descendant combinator between the two type selectors. tsv's parser matches
Svelte byte-for-byte here (`Combinator " "`, span 3–4), so `expected.json` is the
ordinary `div p` AST and this is **not** a `_svelte_divergence`.

## Prettier divergence

tsv normalizes the combinator whitespace to a single space — the same uniform
rule it applies to every combinator gap, where a tab, a newline run, or these
control characters all collapse to `div p` — while prettier freezes the source
byte verbatim, emitting `div<VT>p` / `div<FF>p`. `prettier_variant_vt` and
`prettier_variant_ff` pin the forms prettier keeps stable; tsv normalizes both to
`input.svelte`.

Per CSS Syntax 3 the two characters reach this result from different starting
points:

- **FF (U+000C)** is converted to U+000A LINE FEED during
  [input preprocessing](https://www.w3.org/TR/css-syntax-3/#input-preprocessing),
  so it *is* whitespace — tsv's normalization to a space is spec-aligned and
  prettier's raw-byte preservation is the diverging side.
- **VT (U+000B)** is a
  [non-printable](https://www.w3.org/TR/css-syntax-3/#non-printable-code-point)
  control character that the spec does not count as
  [whitespace](https://www.w3.org/TR/css-syntax-3/#whitespace) (only tab,
  newline, and space). tsv follows Svelte's parser — its drop-in oracle — which
  treats VT as a combinator, and normalizes it uniformly with the rest.

The divergent set is exactly this ASCII C0 pair. NBSP (U+00A0) and the other
non-ASCII Unicode spaces are preserved by tsv (matching prettier), so they do not
belong to this class.

See [conformance_prettier.md §CSS: Selectors](../../../../../docs/conformance_prettier.md#css-selectors).
