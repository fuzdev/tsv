# string_line_continuation_content_prettier_divergence

An inline element (`<span>`) whose expression content is a **line-continuation string literal**
(`'aaaaaaaa\` ⏎ `bbbbbbbb'`) lays out **block-style**: the string's mandatory embedded newline means
the content can never render on one line, so the element expands exactly as it would for width or
structural overflow — both tags intact, the content on its own indented line. The continuation line
itself stays at column 0: the backslash-newline is string content, and re-indenting it would change
the value. Prettier instead **dangles** the tag delimiters (`<span⏎\t>…</span⏎>`) around the leaf.

This is the third block-style trigger — a **leaf's forced newline** — alongside width overflow
(`inline_content_hug_long`) and structural content; the layout stance is identical.

The `unformatted_ours_compact` (hugged authoring) and `prettier_variant_dangle` (prettier's stable
dangle) both normalize to `input.svelte` under tsv in one pass.

## Reason

Uniformity of the block-style stance: the content boundary carries no signal, and a content leaf
that cannot render flat is just another way content goes multiline. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
