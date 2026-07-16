# boundary_nonascii_space_prettier_divergence

A **single** (non-list) CSS value glued to a **non-ASCII whitespace** character — a
non-breaking space (U+00A0) or an em space (U+2003) — at either boundary, e.g.
`content: <NBSP>'z'` (leading) or `quotes: 'a'<NBSP>` (trailing). This is the
single-value sibling of the comma/space-list case
([comma_string_nonascii_space](../lists/comma_string_nonascii_space_prettier_divergence/)).

CSS whitespace is ASCII-only (CSS Syntax 3 §"whitespace" is `\t \n \f \r` and space),
and every non-ASCII code point at U+00A0 and above is a **name** code point, i.e. value
content — never a separator. tsv keeps the value as **one opaque token, inline**,
preserving the character:

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

This pins a former **content loss** — a boundary non-ASCII space was silently dropped
(`content: <NBSP>'z'` → `content: 'z'`). Two sites dropped it, both now fixed:

- **The value-boundary trim** (`value/mod.rs::locate_value`) used the Unicode-aware
  `str::trim*`, which strips a boundary non-ASCII space. It now trims ASCII-only
  (`whitespace::is_ascii_trim_ws`), matching CSS's own whitespace definition — so a
  **trailing** space survives (it is inside the declaration value's extent).
- **The lexer's whitespace scan** treated every `char::is_whitespace()` code point as CSS
  whitespace, so a **leading** space after the `:` was skipped as part of the
  colon→value gap before the value even began. It now excludes the non-ASCII identifier
  code points (U+00A0 and above), so a leading NBSP/em space opens the value's first
  token instead of vanishing.

`input.svelte` is tsv's inline form; `output_prettier.svelte` is prettier's split form.

See [conformance_prettier.md §CSS: Values](../../../../../docs/conformance_prettier.md#css-values).
