# tail_nonascii_space_prettier_divergence

A non-ASCII space (U+00A0) in the tail of an attribute selector: glued to a bare value
(`[attr=value<NBSP>]`), between the value and the case flag (`[attr=value<NBSP>i]`,
`[attr=value<NBSP> s]`, `[attr='value'<NBSP> i]`), after a quoted value (`[attr='value'<NBSP>]`),
and behind the flag (`[attr='value' s<NBSP>]`). `parseCss` reads it as a separator everywhere
in that tail — `read_attribute_value` ends a bare value at JS `\s` and trims,
`REGEX_ATTRIBUTE_FLAGS` reads letters only, and the `allow_whitespace()` between the value,
the flag and the `]` is the same `\s` — so every spelling is the value `value` with the flag
`i` / `s` on the wire. tsv keeps the character where the author put it, as the author
tokenized it; prettier drops it outright (`[attr='value']`, `[attr='value' i]`).

The bytes are the claim because a second reader disagrees. css-syntax-3's whitespace is
ASCII only, so to a browser the run is identifier content and the ASCII space beside it the
token separator: `value<NBSP>` is one ident, `value<NBSP>i` one ident and no flag,
`value<NBSP> s` a value and a flag. Re-quoting the value (`[attr='value'<NBSP>]`) or moving
the run would emit a selector the browser drops where it matched the input — so a bare
value glued to a run stays bare and unquoted, a flag glued to a run stays glued, and ASCII
whitespace beside a run keeps its presence as one space. The tail's other spellings
normalize as usual: extra ASCII whitespace collapses and a double-quoted value takes single
quotes (`unformatted_ours_spaces`, `unformatted_ours_double_quotes`).

## Reason

Content preservation. Dropping a character the author wrote is content loss the corpus
SAFETY check reads as `content_lost`; tsv makes the same call at every other juncture the
parser steps such a run at, and here the author's bytes are the one spelling both tokenizers
read as the input. See
[conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors).
