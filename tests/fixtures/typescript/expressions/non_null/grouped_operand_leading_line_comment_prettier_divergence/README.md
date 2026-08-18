# Non-null grouped operand, line comment in the leading gap

The line-comment sibling of
[grouped_operand_leading_comment](../grouped_operand_leading_comment_prettier_divergence/):
a `//` between the required pair's `(` and the operand (`( // c⏎x + y)!`).

- **tsv**: keeps the comment inside the parens and takes the expanded shell —
  the comment on the `(` line (glued) or its own line (own-line authoring), the
  operand one indent in, the `)!` back out — the rendering every required pair
  in the family takes (the
  [sealed chain](../../chain/optional_paren_non_null_sealed_leading_line_comment_prettier_divergence/),
  the [assignment target](../../assignment/cast_target_leading_line_comment_prettier_divergence/)).

```
const a = ( // c1
	x + y
)!;
```

- **prettier**: hoists the run out of the pair, hanging it in the enclosing gap
  (`const a = // c1⏎(x + y)!;`, `// c3⏎(x as T)! = 1;`) — re-binding it from the
  operand to the whole binding or statement, the same relocation its
  block-comment sibling documents. (At the sealed-chain positions prettier keeps
  the run inside — that fixture's divergence is rendering-only.)

The non-null-over-cast assignment target (`( // c3⏎x as T)! = 1;`) reaches the
same gap and takes the same rendering.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
