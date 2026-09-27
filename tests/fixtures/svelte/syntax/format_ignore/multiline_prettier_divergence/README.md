# multiline_prettier_divergence

A `format-ignore`d node whose bytes span lines, in a block element, an inline element and a
block body. tsv freezes it as it does a `prettier-ignore`d one, and the line break in the frozen
bytes breaks the fragment holding it, which lays out block-style. Prettier does not recognize
`format-ignore`, so it reformats the node onto one line and the fragment stays as authored.

- `output_prettier.svelte` — prettier's output: each frozen element collapsed onto one line, the
  fragments block-style as authored.
- `unformatted_ours_compact.svelte` — each fragment's content glued to its delimiters: tsv
  normalizes it to `input.svelte`, prettier to `variant_compact.svelte`.
- `variant_compact.svelte` — prettier's form of the compact authoring, each element reformatted
  onto one line and the content hugged: both formatters keep it, since no frozen byte spans lines
  there.

## Reason

`format-ignore` is a tsv-native directive, honored alongside `prettier-ignore`; its layout around
a frozen node that spans lines is the `prettier-ignore` one of
[multiline_block_parent](../../prettier_ignore/multiline_block_parent/),
[multiline_inline_parent_long](../../prettier_ignore/multiline_inline_parent_long_prettier_divergence/)
and [multiline_block_body](../../prettier_ignore/multiline_block_body_prettier_divergence/).

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
and [conformance_prettier.md §Reasons tsv Differs](../../../../../../docs/conformance_prettier.md#reasons-tsv-differs).
