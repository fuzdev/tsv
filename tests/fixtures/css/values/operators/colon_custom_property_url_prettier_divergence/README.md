# colon_custom_property_url_prettier_divergence

A URL-shaped custom-property value — `--a: https://a.fuz.dev/b` — stashed in a custom
property rather than wrapped in a `url()`.

tsv: reads the `:` as the top-level colon it is and keeps the rest of the value whole
(`https: //a.fuz.dev/b`)
Prettier: **never reaches a fixed point** (`prettier_nonconvergent.txt`). Its first pass
emits `--a: https: ; //a.fuz.dev/b`, and its own parser throws on that output —
`CssSyntaxError: Unknown word //a.fuz.dev/b`

`◆prettier_bug`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "URL-shaped custom-property value".

## What prettier does

A custom property is exempt from postcss's `checkMissedSemicolon`, so the value is parsed and
its top-level `:` becomes a `colon` node — the same reading that makes `--x: a:1.5` print as
`--x: a: 1.5` ([colon_custom_property](../colon_custom_property/)). What follows the colon is
`//a.fuz.dev/b`, and prettier's value printer reads a leading `//` as an **inline comment** (a
SCSS/Less spelling its CSS printer still recognizes — `isInlineValueCommentNode`): it closes
the declaration ahead of the comment and prints the rest of the URL after the `;`. The result
is not CSS. On the next pass postcss reads `//a.fuz.dev/b` as a word where a declaration is
expected and throws, so no output of prettier's is one it can read back.

## What tsv does

Nothing special: the `:` is a token, `//a.fuz.dev/b` is a word, and the value prints as its
members. A `//` is not a comment in CSS — css-syntax-3 has one comment form, `/* … */` — so
there is nothing here to move out of the value, and every byte the author wrote is still in
it. The `unformatted_ours_compact` variant carries the authored spelling
(`https://a.fuz.dev/b`), which tsv normalizes to input.

The bounding cell is in the fixture: the same URL inside a `url()`, whose content is opaque
(§4.3.6) and which both formatters keep exactly as written.

## Related

- [colon_custom_property](../colon_custom_property/) — the same top-level colon where both
  formatters agree
- [colon_plain_property](../colon_plain_property_prettier_divergence/) — the non-custom
  property, where postcss throws on the input instead
