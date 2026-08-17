# paren_glued_line_comment_prettier_divergence

A **line** comment the author wrote on a unary operand's comment-holder `(` line
(`!( // c`). tsv keeps it there; **prettier moves it to its own line** inside the parens.

```
// tsv                          // prettier
const a = !( // c1              const a = !(
	x || y                              // c1
);                                x || y
                                );
```

## Reason

The unary shell was the **value** family's last holdout on the opening-delimiter rule:
what the author put on an opening delimiter's line stays on it, and what they put on its
own line keeps its own line, so both authorings are fixed points. `return ( // c` and
`throw ( // c` already answered this way
([keyword_open_paren_line_comment](../../../syntax/comments/keyword_open_paren_line_comment_prettier_divergence/)),
as do a retained type paren shell
([paren_shell_glued_leading_line_comment](../../../types/paren_shell_glued_leading_line_comment_prettier_divergence/)),
every bracket and call delimiter, and every statement header
([condition_paren_glued_line_comment](../../../syntax/comments/condition_paren_glued_line_comment_prettier_divergence/)).
Only this one followed prettier — the one-position-agreeing shape that had frozen the rule
at each of the others before it was probed.

Claiming the `(` line is a **break obligation** here, not only a placement: a `//` in the
leading run reaches this shell's group as a real `hardline`, and the glued one has left
that run, so the shell forces itself open instead. Rendered flat it would put the operand
on the comment's own line and the `//` would swallow it.

The shell itself is otherwise unchanged — prettier's own `UnaryExpression` shell
(`group(["(", indent([softline, arg]), softline, ")"])`), which asks nothing about
comments, so the **group still decides flat vs broken on width** for every authoring this
rule does not claim. A run of blocks the author glued owns no line and stays inline
(c11–c12), exactly as before.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **c1** — the plain shape, `!`.
- **c2–c3** — the keyword operators (`typeof`, `void`), which take a space before the
  operand and answer identically.
- **c4** — a sign operator, whose parens the operand genuinely requires rather than being
  a comment holder. The rule does not distinguish them, and neither does the author.
- **c5–c6** — only the **glued** comment claims the `(` line; the rest of the run keeps
  its own lines below it. At most one can, a `//` running to end of line.
- **c7–c8** — the rule's other bound: a **block** glued to the `(` claims nothing, so a
  `//` behind it on that same line has no line to claim either and the whole run stays
  below. Prettier agrees here.
- **c9** — the author blank below the glued comment survives it.
- **c10** — the control: written on its own line the comment keeps its own line, and
  **prettier agrees** (the sibling [rhs_line_comment](../rhs_line_comment/) covers that
  shape on its own). The two formatters part on preserving the author's choice, not on one
  layout.
- **c11–c12** — the second control: a run of blocks the author glued owns no line, so the
  parens still decide their own width and the whole thing stays inline in both formatters
  ([operand_leading_comment_run](../operand_leading_comment_run/)).
