# url_nonascii_space_prettier_divergence

An **unquoted `url()` argument glued to a non-ASCII whitespace** character — a
non-breaking space (U+00A0) — at either end: `url(<NBSP>a)`, `url(a<NBSP>)`,
`url(<NBSP>a<NBSP>)`.

CSS whitespace is ASCII-only (CSS Syntax 3 §4.2), and the `<url-token>` tokenizer
(§4.3.6) consumes exactly that whitespace after the `(` and before the `)`. A non-ASCII
space is not whitespace there — it is a url code point — so the token's value is `<NBSP>a`,
and tsv keeps it:

```
background: url(<NBSP>a);
```

ASCII padding is still trimmed (`unformatted_ours_spaces.svelte`: `url( <NBSP>a )` →
`url(<NBSP>a)`), the same canonical `<url-token>` form as [url_escaped_paren_ws](../url_escaped_paren_ws_prettier_divergence/).

**Prettier drops the character** — its url trim is Unicode-wide, so all three become
`url(a)`. Dropping a character the author wrote is content loss (the corpus SAFETY check
reads it as `content_lost`), and tsv makes the same call here as at every other boundary
non-ASCII space ([boundary_nonascii_space](../../boundary_nonascii_space_prettier_divergence/)):
keep it.

The **quoted** case `url(<NBSP>"a")` is the glued-string design choice of
[comma_string_nonascii_space](../../lists/comma_string_nonascii_space_prettier_divergence/)
seen inside `url()`: both formatters keep the character, but prettier splits the run into a
word plus a string and normalizes the string's quotes (`url(<NBSP>'a')`), where tsv keeps
the glued run as one opaque token, verbatim.

`input.svelte` is tsv's form; `output_prettier.svelte` is prettier's.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values).
