# case_test_gap_own_line_line_comment_prettier_divergence

An **own-line** line comment in a switch case's `case`→test gap
(`case⏎// c1⏎x:`). The gap's other authoring — the comment *trailing* the keyword
(`case // c1⏎x:`) — is
[case_test_gap_line_comment](../case_test_gap_line_comment_prettier_divergence/),
where both formatters keep the comment put and only the indent diverges. Here
the divergence is a **relocation**: prettier pulls the comment up onto the `case`
line, tsv keeps the line the author gave it.

```ts
// tsv (own line kept, test hangs)        // prettier (pulled up, test flush)
switch (a) {                              switch (a) {
	case                                  	case // c1
		// c1                             	x:
		x:                                		b();
		b();                              }
}
```

## Reason

A comment in this gap **leads the test**, and own-line-ness is authoring signal
for a leading position — the corollary in
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy. So the `case`→test gap answers it the way every
other keyword→value gap does (`keyof`/`typeof`, `infer`, a type parameter's
`extends`/`=`, a class-property `=`): a same-line comment trails the keyword, an
own-line comment keeps its own line, and the value hangs one level in below the
run — the shared `append_keyword_value_line_comments` seam. The prefix
type-operator face of the same rule is
[types/type_operator_keyword_line_comment](../../../types/type_operator_keyword_line_comment_prettier_divergence/),
whose own-line form is byte-for-byte this shape.

The head→`:` gap one step later
([case_before_colon_line_comment](../case_before_colon_line_comment_prettier_divergence/))
does **not** take this answer, and the corollary is why: a comment there
*trails the head*, so its own line is layout rather than association, and it
pulls up to the uniform forced-continuation form along with every other
pre-separator gap.

The break itself is not a layout choice on either side. A `//` runs to
end-of-line, so emitting the gap inline and appending the test would **swallow
the test and its `:` into the comment** (`case // c1 x:`), which does not
reparse.

Cases:

- the plain own-line `//` (`case⏎// c1⏎x:`);
- a test whose parens are the **printer's** to re-synthesize
  (`case⏎// c2⏎(y, z):`) — the comment has no node of its own to ride inside, so
  the gap's emitter is the only thing that prints it;
- a **run** (`case⏎// c3⏎// c4⏎w:`), which stays in place, in order, each
  comment on its own line and distinct — emitted with nothing between them a
  `//` welds the next comment into its own text;
- a **block ahead of the line comment** (`case⏎/* c5 */⏎// c6⏎v:`), which keeps
  its own line too rather than being pulled up alone; prettier pulls the block up
  onto the `case` line and leaves the `//` below it, splitting one authored run
  across two positions;
- an author **blank** between the run and the test (`case⏎// c7⏎⏎u:`), which
  survives — the break it separates is forced, so the blank is authoring intent,
  the keyword→value family's rule rather than this gap's.

An own-line **block** comment alone in this gap is not part of this claim — with
no `//` in the gap nothing forces a break, so the comment reflows inline
(`case /* c5 */ x:`) as
[case_test_gap_block_comment](../case_test_gap_block_comment/) records. An
honored `prettier-ignore` reaches the own-line form through the freeze instead of
through this rule, and lands identically —
[case_test_prettier_ignore_head](../case_test_prettier_ignore_head_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy for the principle and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation for the entry.
