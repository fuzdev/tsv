# multiline_block_parent_long_prettier_divergence

A frozen node whose bytes span lines leaves the text after it on the line where the frozen bytes
end, and that line fills from the column they end at. tsv wraps it at the print width like any
other line: a line ending at exactly 100 stays, and one that would end at 101 wraps its last word.

- `output_prettier.svelte` — with a space between the frozen element and the text after it, or
  with the frozen bytes' last line indented, prettier keeps the 101 line whole (it keeps such a
  line longer still, the overrun growing with the last frozen line's indentation). Text glued to an unindented last frozen line wraps at 101 in
  both formatters.

## Reason

Print width. tsv holds the print width on the line after the frozen bytes; the frozen bytes
themselves print as written. The block-element layout around the frozen node is parity
([multiline_block_parent](../multiline_block_parent/)).

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
