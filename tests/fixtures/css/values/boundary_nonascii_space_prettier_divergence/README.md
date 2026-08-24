# boundary_nonascii_space_prettier_divergence

A **single** (non-list) CSS value glued to a **non-ASCII whitespace** character — a
non-breaking space (U+00A0) or an em space (U+2003) — at either boundary, e.g.
`content: <NBSP>'z'` (leading) or `quotes: 'a'<NBSP>` (trailing). This is the
single-value sibling of the comma/space-list case
([comma_string_nonascii_space](../lists/comma_string_nonascii_space_prettier_divergence/)).

CSS whitespace is ASCII-only (CSS Syntax 3 §"whitespace" is `\t \n \f \r` and space),
so a non-ASCII space is never a separator — it is value content (tsv treats every code
point ≥ U+00A0 as an identifier code point, following Svelte's `parseCss`, broader than
the CSS Syntax ident set, which excludes these look-alike whitespace chars). tsv keeps
the value as **one opaque token, inline**, preserving the character:

```
content: <NBSP>'z';
font-family: <NBSP>q;
```

Prettier's value tokenizer instead splits a **string** value glued to the space into two
tokens — a word (`<NBSP>`) and the string (`'z'`) — and inserts a space between them:

```
content: <NBSP> 'z';
```

For an **identifier** value (`font-family: <NBSP>q`) prettier keeps it glued, so tsv
**matches** prettier there; only the string cases diverge. tsv is the more defensible
side on those: it does not split adjacent glued value tokens, so the run stays one token
and its bytes are preserved verbatim — the same lossless form it emits for the pure-ASCII
analog `content: a'y'` (tsv keeps `a'y'`; prettier splits to `a 'y'`) and for the list
case above. Both formatters keep their own output idempotent.

This pins a **content-loss** class — a boundary non-ASCII space silently dropped
(`content: <NBSP>'z'` → `content: 'z'`). Two sites can drop it:

- **The value-boundary trim** (`value/mod.rs::locate_value`) trims CSS whitespace only
  (the escape-aware `escapes::trim_start_css` / `trim_end_preserving_escape`), matching CSS
  Syntax 3 §4.2's ASCII-only whitespace definition — the Unicode-aware `str::trim*` strips
  a boundary non-ASCII space — so a **trailing** space survives (it is
  inside the declaration value's extent).
- **The lexer's whitespace scan** excludes the non-ASCII identifier code points (U+00A0
  and above) — treating every `char::is_whitespace()` code point as CSS whitespace skips a
  **leading** space after the `:` as part of the colon→value gap before the value even
  begins — so a leading NBSP/em space opens the value's first token instead of vanishing.

`input.svelte` is tsv's inline form; `output_prettier.svelte` is prettier's split form.

See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values).
