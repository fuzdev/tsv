# combinator_comment_svelte_prettier_divergence

Comments are inter-token whitespace per [CSS Syntax 3 §comments](https://www.w3.org/TR/css-syntax-3/#comment-diagram)
— valid wherever whitespace is, including inside a complex selector at a
combinator boundary. tsv accepts them in every combinator position:

- descendant gap — `div /* c */ p`
- explicit combinator, comment after — `a > /* c */ b`
- explicit combinator, comment before — `i /* c */ > em`
- compound-internal, glued — `.a/* c */.b` (no whitespace → stays a compound,
  not a descendant `.a .b`)
- relative-selector leading combinator in `:has()` — `:has(> /* c */ img)`

## Svelte Behavior

Svelte's `parseCss` rejects a comment at a combinator boundary: its selector
scanner does not tokenize comments there and reports "Expected a valid CSS
identifier" (`css_expected_identifier`). This is the canonical-fails-tsv-ok
pattern — tsv follows the CSS spec where Svelte's parser is incomplete, the same
shape as [no_namespace](../namespace/no_namespace_svelte_divergence/) and
[attribute namespace](../attribute/namespace_svelte_divergence/).

## Prettier divergence

tsv normalizes the gap spacing around the comment to a single space — the same
rule as every other selector-comment position (`:is()`/`:nth-*()`/`::slotted()`
argument comments) — while prettier freezes the source whitespace verbatim.
`prettier_variant_spaces` pins the padded forms prettier keeps stable; tsv
normalizes them to `input.svelte`. The compound-internal glued form (`.a/* c */.b`)
has no gap whitespace to normalize, so it stays glued under both formatters — a
space there would turn the compound into a descendant `.a .b`, so it is never
added.

## tsv Behavior

tsv parses these per the spec and preserves the comment in its authored position,
registering it at parse time and re-emitting it through `comments_in_range` so the
surrounding whitespace normalizes uniformly (the shared selector-comment doc path).
The compound-vs-descendant distinction the whitespace carries is never altered.

See [conformance_prettier.md §CSS: Comments](../../../../../docs/conformance_prettier.md#css-comments)
and [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).
