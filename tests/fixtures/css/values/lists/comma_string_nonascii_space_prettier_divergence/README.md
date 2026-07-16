# comma_string_nonascii_space_prettier_divergence

A comma- or space-separated CSS value with an element that is a **quoted string
glued to a non-ASCII whitespace** — a non-breaking space (U+00A0) or an em space
(U+2003) — on either side, e.g. `grid-template-areas: 'x', <NBSP>'y'` (leading) or
`grid-template-columns: 'a'<NBSP>, 'b'` (trailing).

CSS whitespace is ASCII-only (CSS Syntax 3 §"whitespace" is `\t \n \f \r` and
space), so a non-ASCII space is **not** a separator — it is a name code point, i.e.
value content. tsv keeps the element as **one opaque token, inline**, preserving the
character:

```
grid-template-areas: 'x', <NBSP>'y';
```

Prettier's value tokenizer instead splits `<NBSP>'y'` into two tokens — a word
(`<NBSP>`) and a string (`'y'`) — which makes the group multi-token, so it (a)
inserts a space between them and (b), for a comma list, breaks it one item per line
(its `shouldBreakList`):

```
grid-template-areas:
	'x',
	<NBSP> 'y';
```

tsv is the more defensible side: it does not split adjacent glued value tokens, so
the run stays one token and its bytes are preserved verbatim — the same lossless
form it emits for the pure-ASCII analog `font-family: 'x', a'y'` (tsv keeps `a'y'`;
prettier splits to `a 'y'`). Both formatters keep their own output idempotent.

This pins a former **content loss**. CSS whitespace is ASCII-only, but the value
parser's element-boundary trims and the printer's value-text whitespace normalizer
both used the Unicode-aware `str::trim`, which strips a boundary non-ASCII space. For
a **string** element that was outright deletion: the trim left the string's span
covering the space while narrowing its text, so the string printer extracted a span
that no longer began with a quote and emitted nothing — the whole element vanished
(`'x',;`, then `'x';` on a second pass — non-idempotent). An ident element lost just
the space. The fix trims only CSS whitespace (`trim_start_css` /
`trim_end_preserving_escape`, matching CSS Syntax 3 §4.2's ASCII-only definition)
everywhere a value boundary is trimmed, so the non-ASCII space survives as content.

`input.svelte` is tsv's inline form; `output_prettier.svelte` is prettier's split
form.

See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values).
