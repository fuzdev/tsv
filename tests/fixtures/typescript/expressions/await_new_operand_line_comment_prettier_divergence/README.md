# `await`→operand / `new`→callee gap, forced continuation indent

The `//` spelling of the gap the block-comment sibling
[await_new_operand_own_line_block_comment](../await_new_operand_own_line_block_comment_prettier_divergence/)
covers. A line comment runs to end-of-line, so the operand cannot stay on the
keyword's line — the break is forced, and the only question left is how far the
continuation indents.

- **tsv**: one level in, the answer every other forced continuation gives
  ([§Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)),
  so the operand reads as part of its construct rather than as a sibling statement.
- **prettier**: flush at the keyword's own column (`new // c1⏎Foo();`), where
  `Foo();` reads as the next statement.

```
new // c1        new // c1
	Foo();       Foo();
```

The same keyword one token over already indents: `new // c⏎\t.target` is
[meta_property/dot_gap_line_comment](../misc/meta_property/dot_gap_line_comment_prettier_divergence/).

What the cases pin, beyond the plain `new`/`await` continuation (c1, c2, c3):

- **c4** — an operand whose parens are **required** (`await` binds tighter than
  `+`) keeps the run **outside** the pair. The gap is the keyword's, not the
  parens', so it answers the same way whether or not the operand happens to need
  them; prettier keeps the run outside here too, so only the indent diverges. A
  comment the author wrote *inside* the parens is the separate claim in
  [grouped_operand_comment](../await_yield/grouped_operand_comment_prettier_divergence/).
- **c5/c6** — a run stays one comment per line, all of it on the continuation.
- **c7** — an own-line **multiline** block hangs for its own reason (inlining it
  would reflow the author's break) and takes the same indent, matching the
  `keyof`/`extends`/`=` siblings that share this shape.
- **c8** — in value position the continuation is one level under the statement.
  Prettier additionally breaks the `=` there; tsv keeps the `await` on the `=`
  line, so this case diverges in two ways at once.
- **c9** — the control: a single-line block forces nothing, collapses inline, and
  has no continuation to indent (the block sibling's rule).

`yield` is **not** a site of this rule — a newline after it is ASI, so the operand
becomes its own statement ([rhs_line_comment](../await_yield/rhs_line_comment/)),
and the unary operators take a comment-holder shell instead
(`typeof (⏎// c⏎x⏎)`).

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
