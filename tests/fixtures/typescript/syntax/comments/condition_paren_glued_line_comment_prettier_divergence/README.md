# condition_paren_glued_line_comment_prettier_divergence

A **line** comment the author wrote on a statement header's `(` line (`if ( // c`). tsv
keeps it there; **prettier moves it to its own line** inside the parens — or, at do-while,
out of the parens entirely.

```
// tsv                          // prettier
if ( // c1                      if (
	a                                   // c1
) {                               a
	b();                          ) {
}                                 b();
                                }
```

## Reason: it is the opening-delimiter rule, and the statement headers were the last holdout

What the author put on an opening delimiter's line stays on it, and what they put on its
own line keeps its own line — so both authorings are fixed points. tsv already answered
this way at every other delimiter, and diverges from prettier at every one of them:
`fn( // c`, `new Foo( // c`, `[ // c`, `{ // c`, `Array< // c`, `function f( // c` and a
retained type paren shell's `( // c` all keep the comment where it was written, while
prettier drops it to the next line at all of them
([open_paren_comment](../../../expressions/calls/open_paren_comment_prettier_divergence/),
[paren_shell_glued_leading_line_comment](../../../types/paren_shell_glued_leading_line_comment_prettier_divergence/)).
The statement headers were the one family that followed prettier instead, so a `(` opening
a condition answered the question differently from the `(` opening a call two lines above
it — and do-while, which shares the very same builder, already glued.

Prettier is not a coherent oracle for this position: it un-glues at `if` / `while` /
`switch` / `catch` / the C-style `for` (a side effect of its comment-attach handlers
binding the comment to the test), **glues** at for-in / for-of, and relocates the comment
out of the parens at do-while. One position agreeing was what froze the rule
([conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)).

The association the position carries is real once the condition is more than a name: a
comment on the `(` line is about the test, one on its own line above the first operand is
about that operand.

An author **blank line** below the glued comment rides along, through the same emitter a
retained paren shell uses: which line the comment sits on and how far the author separated it
from the condition are two different facts. Prettier keeps that blank too, at its own
un-glued placement. A blank between the `(` and the comment is a different question and stays
erased — it sits against the delimiter, where tsv and prettier drop it at every bracket alike.
⚠️ The blank half is not the delimiter family's shared answer — the list and call families
drop it, and the split is unresolved
([comments.md §The delimiter-line question](../../../../../../docs/comments.md)).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **c1–c2** — `if` and `while`, the plain shape. Prettier keeps the comment inside the
  parens and only moves it off the `(` line.
- **c3** — do-while, which already answered this way: prettier relocates the comment out of
  the parens entirely (past the `;` for a `//`), so it is no oracle there at all
  ([open_paren_comment](../../../statements/do_while/open_paren_comment_prettier_divergence/)).
- **c4** — `switch`, whose discriminant parens share the condition builder.
- **c5** — `catch`, whose parameter parens do too.
- **c6–c7** — the author blank below the glued comment, at a construct where prettier keeps
  it (`if`) and at one where it does not (do-while, having moved the comment out).
- **c8–c10** — only the **glued** comment claims the `(` line; the rest of the run keeps
  its own lines below it, through the shared leading emitter. At most one comment can
  claim the line, since a `//` runs to end of line.
- **c11–c12** — the rule's other bound: a **block** glued to the `(` claims nothing, so a
  `//` behind it on that same line has no line to claim either and the whole run stays
  below the `(`. Prettier agrees here.
- **c13** — the control: written on its own line the comment keeps its own line, and
  **prettier agrees**. The two formatters part on preserving the author's choice, not on
  one layout.
- **c14** — the second control: a lone **block** comment is untouched by this rule. It
  collapses inline in both formatters, exactly as it does at `fn(` and `[`, because
  nothing forces the header open around it.
