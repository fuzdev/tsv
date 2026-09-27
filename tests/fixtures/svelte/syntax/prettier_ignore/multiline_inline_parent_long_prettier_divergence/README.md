# multiline_inline_parent_long_prettier_divergence

A frozen node whose bytes span lines, inside an inline element or a component. The line break in
the frozen bytes breaks the element holding it — as prettier's `literalline` breaks every group
around a frozen template node — so tsv lays the element out block-style: the content on its own
indented line, the frozen bytes as written. The text after the frozen node fills from the
column where the frozen bytes end: a line ending at exactly 100 stays, and the word that would end
it at 101 wraps. Every inline ancestor of the frozen node breaks with it, so an element nested
in another lays out block-style at both levels. A freeze whose bytes fit on one line, and the same
element unfrozen, are the controls: neither spans lines, so both stay inline.

- `unformatted_spaces.svelte` — spaces at the content boundaries: both formatters normalize it to
  `input.svelte`.
- `unformatted_ours_compact.svelte` — each element's content glued to its tags: tsv normalizes it
  to `input.svelte`, prettier dangles the element's delimiters around it.
- `unformatted_ours_hug.svelte` — the content hugged by its tags, the long text wrapped against
  the closing tag: tsv normalizes it to `input.svelte`, prettier dangles the delimiters.
- `prettier_variant_dangle.svelte` — prettier's form of the compact authoring, the delimiters
  dangled around the content: prettier keeps it, tsv normalizes it to `input.svelte`.

All of them render identically.

## Reason

Design choice and convergence, render-free under Svelte 5. Content that renders over several
lines lays an inline element out block-style however the author wrote its content boundary, which
the compiler trims; a frozen node that spans lines is such content. Prettier keeps the block-style
form too, but holds the dangled one beside it for a glued authoring.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive),
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style),
and [conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).

## Related

- [multiline_block_parent](../multiline_block_parent/) — the same freeze in a block element,
  where prettier agrees
- [multiline_block_body](../multiline_block_body_prettier_divergence/) — the same freeze in a
  block body
