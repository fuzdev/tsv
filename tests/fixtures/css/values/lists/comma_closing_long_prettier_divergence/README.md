# comma_closing_long_prettier_divergence

The width axis of [comma_closing](../comma_closing_prettier_divergence/): a comma
**closing** a value costs a column, and two printer paths have to account for it.
Each is pinned at its own 100/101 boundary. Prettier deletes the comma in all four
declarations and — a column shorter — then never wraps at all.

**A value carrying a comment** isn't printed from the AST (CSS value comments
aren't stored there) but from source, on a path with two width-chosen branches:

| width | branch | form |
| --- | --- | --- |
| 100 | inline — the value is re-emitted normalized | `linear-gradient(…, #555 100%,)` |
| 101 | wrapped — the arguments are split from source, one per line | closing comma on the last argument's line |

The wrapped branch splits on top-level commas, and CSS Syntax 3 §"parse a
comma-separated list of component values" produces no part for a comma in final
position, so the comma is written back after the parts rather than recovered from
one.

**A broken comma list** puts one element per line, and its last element's line ends
`,;` — two columns, not one. The element reserves both, so one that would land at
exactly 101 wraps at its own space instead of overrunning; at 100 it stays flat.
tsv treats the print width as a hard limit, so a reservation one column short is an
overrun, not a rounding difference.

`unformatted_ours_flat` is the same content written flat, with the authored space
kept after each closing comma (`…, )`) — tsv normalizes both away, prettier deletes
the commas instead, so the variant is `_ours`.

## Reason

**Content preservation** — the same divergence as
[comma_closing](../comma_closing_prettier_divergence/), seen at the width boundary.
Deliberately **not** ◆print_width: neither formatter overruns here. Prettier deletes the
comma and, a column shorter, never reaches the width at all. See
[conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values)
("Closing comma in a value").

## Related

- [comma_closing](../comma_closing_prettier_divergence/) — the rule, at every other position
- [comma_trailing_empty_element](../comma_trailing_empty_element_prettier_divergence/) — one comma further, where the last element is empty
