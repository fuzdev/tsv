# case_test_gap_line_comment_prettier_divergence

A line comment in a switch case's `case`→test gap (`case // c⏎x:`). Both
formatters keep every comment exactly where the author wrote it — placement,
order and kind all match — so the divergence is the continuation's **indent**
alone: tsv drops the test one level in (uniform forced-continuation indent),
prettier leaves it flush at the case's own indent.

The break itself is not a layout choice. A `//` runs to end-of-line, so
emitting the gap inline and appending the test would **swallow the test and its
`:` into the comment** (`case // c x:`), which does not reparse — the same
content-preservation argument as this construct's head→`:` gap
([case_before_colon_line_comment](../case_before_colon_line_comment_prettier_divergence/))
and the switch `)`→`{` gap
([head_body_line_comment](../head_body_line_comment_prettier_divergence/)).
Only where the tail lands is open, and tsv answers it the same way at every
head→tail gap a `//` splits.

```ts
// tsv (continuation indent)              // prettier (flush)
switch (a) {                              switch (a) {
	case // c                             	case // c
		x:                                	x:
		b();                              		b();
}                                         }
```

Cases, all four sharing the one rule:

- the plain test (`case // c⏎x:`);
- a test whose parens are the **printer's** to re-synthesize
  (`case // c2⏎(y, z):`) — the comment has no node of its own to ride inside,
  so the gap's emitter is the only thing that prints it;
- a **run** (`case // c3⏎// c4⏎w:`), which stays in place, in order, each
  comment on its own line and distinct — emitted with nothing between them a
  `//` welds the next comment into its own text;
- a **block ahead of the line comment** (`case /* c5 */ // c6⏎v:`), which keeps
  its place in the run rather than reordering across it (a block prints inline
  where a line comment defers, so a run emitted out of order would put the
  block behind the `//` it was written in front of).

The same gap's **block** comment collapses inline and matches prettier
([case_test_gap_block_comment](../case_test_gap_block_comment/)) — a block
forces no break, so nothing splits the head from its tail and no continuation
exists to indent.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent for the principle and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation for the entry.
