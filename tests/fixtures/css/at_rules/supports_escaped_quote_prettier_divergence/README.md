# supports_escaped_quote_prettier_divergence

An at-rule prelude whose value carries an **escaped quote** beside a real string —
`@supports (a: x\"y 'z')`.

Per CSS Syntax 3 §4.3.7 the `\"` is a valid escape whose escaped code point is the quote
itself, and §"Consume an ident sequence" takes it as ident content: `x\"y` is the single
ident `x"y`, and it opens no string. The `'z'` beside it is the only string in the prelude, so
it takes prettier's quote normalization like any other (`"z"` → `'z'`).

Prettier's prelude normalizer reads the escape's payload **byte** as a delimiter: the `\"`
opens a string that runs to the *next* real quote, so the whole region between them is
swallowed and the declaration comes back unnormalized (`prettier_variant_double_quotes` is a
prettier fixed point). Under `@media` the same blindness is worse — prettier re-quotes the
swallowed region and emits `@media (a: x\'y 'z")`, which rewrites the escape's payload,
moves the string delimiters, and no longer parses.

tsv steps every escape whole before any of its prelude arms run, so the escape's payload can
never be the delimiter, the hash, or the comment introducer one of them is looking for. Same
rule and same reason as [CSS: Values §Escaped whitespace in a value](../../../../../docs/conformance_prettier_css.md#css-values):
output that does not re-parse is never the defensible side.

No `output_prettier.*`: on tsv's canonical form (`input`) prettier agrees, so the divergence
lives only in `prettier_variant_double_quotes` — a form prettier keeps stable that tsv
normalizes to `input`. The sibling
[supports_escaped_hash_separator](../supports_escaped_hash_separator_prettier_divergence/) is the
same blindness one delimiter over, where prettier's output *does* differ on tsv's own form.

See [conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules).
