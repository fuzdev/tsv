# retained_paren_shell_trailing_comment_run_prettier_divergence

A **run** of two or more comments in a **retained paren shell's trailing gap** — the
region between the type a `(`…`)` wraps and its `)`, where the shell prints its own
parens. The single-comment case is pinned by
[retained_paren_intersection_member_comment](../retained_paren_intersection_member_comment_prettier_divergence/)
(the intersection-with-trailing-object shell) and
[union_intersection_retained_paren_line_comment](../union_intersection_retained_paren_line_comment_prettier_divergence/)
(the union shell); this fixture pins what happens when that gap holds more than one.

**Prettier** hoists the run out of the parens — the first comment trails the whole
member, the rest lead the next one (`| (a & { x: X }) // c1` then `// c2`). **tsv**
keeps every comment inside the shell, per Comment Position Philosophy, the same rule
the single-comment siblings above record.

The divergence is therefore only **placement**, which those siblings already sanction.
What this fixture adds is the property both formatters must share: the comments stay
**distinct**. A run emitted with nothing between its comments welds them into one — the
second `//` becomes text of the first (`// c1 // c2`), so the second comment stops
existing. Each comment after a **line** comment therefore takes a break before it,
because a `//` runs to end of line and cannot share it:

- `A1` — two line comments in the intersection shell's gap. The continuation sits at the
  `)` column (the `align(2)` sub-tab offset under the `(`) that the closer already takes,
  so everything below the `(` line closes in one column.
- `A2` — three comments, same rule; the run has no special last element.
- `A3` — a **block** comment after a line comment. Emitted with nothing between them the
  block also **reorders** ahead of the line comment, because a block is inline while a
  line comment rides `line_suffix`; the break restores the authored order.
- `A4` — the control: two **glued** block comments, which share a line in the source and
  keep sharing it. A block comment can hold its line, so no break is added and the member
  stays inline. The separator is conditional on what precedes it, not blanket.
- `B1` — the union shell's gap. The run lands at the shell's **interior** indent here
  rather than the `)` column, because this shell emits its trailing gap inside the
  indented interior while the intersection shell emits its own outside the object's
  group. Each run continues at the column of the region its shell emits it into.
- `B2` — the same union shell as an array element, where the `)` sits at the base indent.

Prettier's own output keeps every comment distinct too, so it is the oracle for that
half even though it is not the oracle for placement. In `B2` prettier likewise keeps the
run inside the parens on its own line; it differs only by exploding the inner union,
the layout difference `union_intersection_retained_paren_line_comment` already records.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
