# property_nonascii_space_prettier_divergence

A non-ASCII space (U+00A0) in a declaration's property→colon gap: glued to the name
(`color<NBSP>: red`), separated from it by a space (`top <NBSP>: 0`), and in a
comment-bearing gap on either side of the comment (`left<NBSP> /* comment */ : 0`,
`right /* comment */<NBSP> : 0`). `parseCss` ends the property at the run —
`read_declaration` reads the name with `read_until(/[\s:]/)`, JS `\s`, and then
`allow_whitespace()`s to the colon — so every spelling is the property `color` / `top` /
`left` / `right` on the wire. tsv keeps the character where the author put it, ahead of the
separator it regenerates (the `: ` after a name, the single spaces around a property
comment), with the ASCII space ahead of the run kept as one space: css-syntax-3's whitespace
is ASCII only, so to a browser `color<NBSP>` is one ident and `top <NBSP>` an ident and a
stray token, and only the author's spelling tokenizes the same way under both readers.

Prettier keeps a run glued to what precedes it (`color<NBSP>: red`, `left<NBSP>/* comment */`)
but **drops** one an ASCII space separates from the name (`top <NBSP>: 0` → `top: 0`), and
drops the space before a property comment — the `in_property_value_before_colon` rule.

## Reason

Content preservation, plus the comment-spacing rule. Dropping a character the author wrote
is content loss the corpus SAFETY check reads as `content_lost`; tsv makes the same call at
every other juncture the parser steps such a run at. The single-space normalization around
the property comment is the `in_property_value_before_colon` quirk. See
[conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments)
and the boundary-whitespace entry in
[conformance_prettier_css.md §CSS: Selectors](../../../../../docs/conformance_prettier_css.md#css-selectors).

## Related

- [in_property_value_before_colon](../../tokens/comments/in_property_value_before_colon_prettier_divergence/) — the comment-spacing half, ASCII only
