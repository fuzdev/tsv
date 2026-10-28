# Non-null grouped operand then member access, line comments in both gaps

The both-gaps sibling of
[grouped_operand_member_line_comment](../grouped_operand_member_line_comment_prettier_divergence/)
and [grouped_operand_leading_line_comment](../grouped_operand_leading_line_comment_prettier_divergence/):
a `//` glued to the `(` of a retained `)!` shell **and** a `//` before its `)`
(`(// c⏎x.m().m().c(() => d) // t⏎)!.f()`), with an operand that breaks.

- **tsv**: one expanded shell — the leading `//` on the `(` line, the operand
  one indent in with its own continuation (a chain's links, a binary's later
  operands, two in), the trailing `//` beside the operand's last line, the `)!`
  back out.

```
( // c1
	x
		.m()
		.m()
		.c(() => d) // t1
)!.f();
```

- **prettier**: relocates both comments out of the pair. For the chain operand
  (c1) it strips the pair, moves the leading comment into the chain
  (`.c// c1⏎(() => d)`) and the trailing one past the operand, and that output
  does not reparse (`// t1⏎!.f();`) — so `output_prettier.svelte` is its one pass,
  with no fixed point to compare with. For the binary operand (c2) it hoists the
  leading comment above the statement and moves the trailing one past the `)!`
  (`// c2⏎(a + // m⏎\tb)! // t2⏎\t.f();`).

The `unformatted_ours_glued` variant holds the authored compact spellings, which
tsv normalizes to this file in one pass.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Non-null grouped operand then member access) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
