# block_glued_before_prettier_divergence

A directive glued to the text in front of the block element it freezes. tsv keeps the directive
glued to the element; prettier breaks between them. After the element both formatters give the
boundary the break the element gets unfrozen, whether the author wrote a space or a line break
there.

- `output_prettier.svelte` — prettier's output: each directive and its element on separate lines.
  tsv keeps that form too, since the authored break is the gap it re-emits.
- `unformatted_ours_space_after.svelte` — every element followed by a space: tsv normalizes it
  to `input.svelte`, prettier to `output_prettier.svelte`.

All of them render identically.

## Reason

Design choice. The gap in front of a frozen node is the author's and is never invented: a
directive glued to its node stays glued at every frozen kind, where the break would inject a
rendered space before an inline one — the one rule, not a per-kind layout. The boundary after the
element is not the freeze's: it is the printer's, and it takes the form the element takes there
unfrozen.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Related

- [directive_gap_glued](../directive_gap_glued/) — the glued gap in front of inline kinds, where
  prettier agrees
- [block_space_after](../block_space_after/) — the boundary after a frozen block element whose
  directive owns its line, where prettier agrees
