# header_paren_glued_line_comment_prettier_divergence

A **line** comment the author wrote on a `for` header's `(` line (`for ( // c`). tsv keeps
it there; **prettier moves it to its own line** in the C-style header, and in a for-in /
for-of header keeps the `(` line but collapses the whole head onto the comment's next line
at **column zero**.

```
// tsv                          // prettier
for ( // c1                     for (
	let i = 0;                          // c1
	i < n;                            let i = 0;
	i++                               i < n;
) {                                 i++
	b();                            ) {
}                                   b();
                                  }

for ( // c2                     for (// c2
	const x                       const x of arr) {
		of                            b();
		arr                         }
) {
	b();
}
```

## Reason

The `for` face of the opening-delimiter rule — what the author put on an opening
delimiter's line stays on it, so both authorings are fixed points. The shared statement of
the rule, and why the statement headers were the family's last holdout, is
[condition_paren_glued_line_comment](../../../syntax/comments/condition_paren_glued_line_comment_prettier_divergence/);
this fixture pins the three `for` spellings, which reach the gap through their own header
builders rather than through the condition group.

Prettier answers the same position two ways inside this one statement family, which is the
clearest evidence that its placement here is a side effect of comment attachment rather
than a rule: in the C-style header it drops the comment to its own line, and in a for-in /
for-of header it **glues** it to the `(` and then emits the rest of the head unindented at
column zero. tsv writes the same form for all three.

An author **blank line** below the glued comment rides along (c4), through the emitter the
condition headers and the type paren shells share; prettier keeps it in the C-style header
too. The blank half is not the whole delimiter family's answer — see the shared statement.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **c1** — the C-style header, whose clauses take their own lines below the comment.
- **c2–c3** — for-of and for-in, which share one header builder. The binding / keyword /
  iterable layout below the comment is the header's existing broken form
  ([in_of_header_comment_run](../in_of_header_comment_run_prettier_divergence/)); only the
  comment's line changes here.
- **c4** — the author blank below the glued comment.
- **c5** — the control: written on its own line it keeps its own line, and **prettier
  agrees**.
