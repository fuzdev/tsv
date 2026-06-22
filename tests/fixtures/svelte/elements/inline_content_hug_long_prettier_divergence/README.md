# inline_content_hug_long_prettier_divergence

An inline element (`<small>`) whose **expression content** overflows print width lays out
**block-style**: both tags stay intact and the content moves to its own indented line, exactly like
a block element (and like the text case `inline_content_text_wrap`). Content that fits stays inline
(at 100 chars); only overflow breaks to block-style.

Giving the content its own indented line is usually enough room to keep the expression **whole** —
ternaries, logical (`&&`) chains, and binary (`+`) chains all sit unbroken on that line. Prettier
instead **dangles** the tag delimiters (`<small⏎\t>…</small⏎>`) and, at the tighter dangled indent,
breaks the expression operator-by-operator (one `&&`/`+`/ternary per line — see
`prettier_variant_compact`).

When the content is unbreakable identifiers, both formatters reach the same width regardless; the
companion `inline_content_unbreakable` covers that case.

The `unformatted_ours_compact` (single-line authoring) and `prettier_variant_compact` (prettier's
dangle) both normalize to `input.svelte` under tsv in one pass.

## Reason

tsv treats printWidth as a hard limit and prefers block-style content over a dangled, operator-broken
expression: it keeps `<tag>` and `</tag>` intact and the expression visually whole on one indented
line. In real-world cases this keeps lines well within printWidth where prettier's dangle pushes them
over. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
