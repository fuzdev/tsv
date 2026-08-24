# `await`→operand / `new`→callee gap, own-line comment run

The **own-line** authoring of the gap
[await_new_operand_line_comment](../await_new_operand_line_comment_prettier_divergence/)
covers trailing the keyword. There the comment stays put in both formatters and
only the continuation's indent diverges; here prettier **relocates** — it pulls
the first comment up onto the keyword's line — while tsv keeps the line the
author gave it.

```
new              new // c1
	// c1        Foo();
	Foo();
```

## Reason

A comment in this gap **leads the operand**, and own-line-ness is authoring
signal for a leading position — the corollary in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy. `new`→callee and `await`→operand are the two
expression-level keyword→value gaps, so they take the keyword→value family's
answer (`append_keyword_value_line_comments`): a same-line comment trails the
keyword, an own-line comment keeps its own line, and the whole tail — callee,
type arguments and argument list — hangs one level in below the run. The same
shape at `keyof`/`typeof`
([type_operator_keyword_line_comment](../../types/type_operator_keyword_line_comment_prettier_divergence/))
and at a switch label's `case`→test
([case_test_gap_own_line_line_comment](../../statements/switch/case_test_gap_own_line_line_comment_prettier_divergence/)).

The break itself is not a layout choice on either side: a `//` runs to
end-of-line, so inlining the gap would swallow the operand.

What the cases pin:

- **c1/c2** — the plain `new` and `await` forms, one gap under two keywords.
- **c3** — an operand whose parens are **required** (`await` binds tighter than
  `+`) keeps the run **outside** the pair. The gap is the keyword's, not the
  parens'. A comment the author wrote *inside* those parens is the separate claim
  in [grouped_operand_comment](../await_yield/grouped_operand_comment_prettier_divergence/).
- **c4/c5** — a run keeps one comment per line, in order; prettier pulls only the
  first up and leaves the rest below, splitting one authored run across two
  positions.
- **c6/c7** — a **block ahead of the line comment** keeps its own line too, where
  prettier pulls the block up alone.
- **c8** — an own-line **multiline** block reaches this arm through the gate's
  broke-after half rather than through a `//`, and lands the same way. The
  *glued* multiline authoring (`new /* … */⏎Foo()`) is on the keyword's line, so
  it trails there — the trailing sibling's `c7` case.
- **c9** — in value position the run sits one level under the statement.
  Prettier additionally breaks the `=` there, so this case diverges twice over.
- **c10** — the control: a comment authored **on** the keyword's line trails it in
  both formatters, and only the indent diverges (the trailing sibling's rule).

`yield` is **not** a site of this gap — a newline after it is ASI, so the operand
becomes its own statement ([rhs_line_comment](../await_yield/rhs_line_comment/))
— and the unary operators take a comment-holder shell instead
(`typeof (⏎// c⏎x⏎)`). A single-line block in any authored position forces
nothing, collapses inline, and is the
[await_new_operand_own_line_block_comment](../await_new_operand_own_line_block_comment_prettier_divergence/)
claim.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
