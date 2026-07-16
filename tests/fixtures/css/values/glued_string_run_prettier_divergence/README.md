# glued_string_run_prettier_divergence

A single CSS declaration value that is a **run of glued value tokens** whose first
and last token are quoted **strings** with no ASCII whitespace between them, e.g.
`content: 'a''b'` (two adjacent strings), `content: 'a'x'b'` (string, identifier,
string), or `content: "a"x"b"` (double-quoted). This is the pure-ASCII sibling of
the non-ASCII-space glued cases
([comma_string_nonascii_space](../lists/comma_string_nonascii_space_prettier_divergence/),
[boundary_nonascii_space](../boundary_nonascii_space_prettier_divergence/)) — here the
tokens are simply written adjacent, with no separator at all.

tsv does not split adjacent glued value tokens, so it keeps the run as **one opaque
token, inline**, preserving its bytes verbatim:

```
content: 'a''b';
content: 'a'x'b';
content: "a"x"b";
```

Prettier's value tokenizer instead splits the run into its component tokens and
inserts a space between them (also normalizing the quote to `'`):

```
content: 'a' 'b';
content: 'a' x 'b';
content: 'a' x 'b';
```

tsv is the more defensible side — the same lossless verbatim form it emits for the
pure-ASCII analog `font-family: 'x', a'y'` (tsv keeps `a'y'`; prettier splits to
`a 'y'`). Both formatters keep their own output idempotent.

This pins a former **content corruption**. The value re-parser's
`parse_string_literal` classified the run as a single string whenever its first
character was a quote and its last character was the matching quote — true for
`'a'x'b'` even though the opening `'` is closed at index 2, not at the end. It then
stripped the outer quotes and re-quoted the interior (`a'x'b`) optimally, turning
the **delimiter** `'`s into literal content: `content: 'a'x'b'` became
`content: "a'x'b"` — a value whose CSS meaning changed (three components collapsed
into one string; the parse round-trips to a different AST). The fix requires the
opening quote's first **unescaped** matching close to fall at the end of the run
(a genuine single string spanning the whole value); otherwise the run is kept
verbatim as an opaque identifier, matching the CSS tokenizer (string + ident +
string). An escaped interior quote (`'a\'b'`) still closes at the end, so a real
single string is unaffected.

`input.svelte` is tsv's verbatim form; `output_prettier.svelte` is prettier's split
form.

See [conformance_prettier.md §CSS: Values](../../../../../docs/conformance_prettier.md#css-values).
