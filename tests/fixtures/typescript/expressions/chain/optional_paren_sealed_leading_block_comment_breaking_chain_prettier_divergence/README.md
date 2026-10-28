# Sealed chain base, glued leading block comment, breaking chain

The block-comment sibling of
[optional_paren_sealed_leading_line_comment_breaking_chain](../optional_paren_sealed_leading_line_comment_breaking_chain_prettier_divergence/):
a single-line block glued through a sealed chain base's leading gap
(`(/* c */ (a as B) // m⏎?.q).m`), with a chain that breaks inside the pair.

- **tsv**: the block leads the operand on the operand's own line inside the
  base's expanding parens — the comment-free base's layout (`(⏎\t(a as B)…⏎).m`)
  with the block in front of the operand, which is also where the reparse finds
  it, glued to the operand.

```
(
	/* c1 */ (a as B) // m
		?.q
).m;
```

- **prettier**: keeps the block on the `(` line and the continuation flush
  (`(/* c3 */ (a as B)⏎\t?.m()…).z;`), and, before a trailing `//`, breaks inside
  a cast operand's own parens instead (`(/* c1 */ (⏎\ta as B⏎) // m⏎?.q).m;`) —
  the block stays in the pair either way, so the divergence is the rendering
  alone.

A kept `)!` seal (c5) and a `throw` argument (c6) take the same layout. A block
trailing the breaking chain stays beside its last link, inside the parens
(c7), where prettier puts it too. When the
chain fits, the block leads it flat on the `(` line and both formatters agree
byte-for-byte (c4, the control). The `unformatted_ours_glued_paren_shell`
variant holds the authored compact spellings, which tsv normalizes to this file in one
pass.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
