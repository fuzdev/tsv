# Sealed chain base, leading line comment, breaking chain

The breaking-body sibling of
[optional_paren_non_null_sealed_leading_line_comment](../optional_paren_non_null_sealed_leading_line_comment_prettier_divergence/)
and [optional_paren_sealed_line_comment](../optional_paren_sealed_line_comment_prettier_divergence/):
a `//` (or own-line comment) in the leading gap of a sealed chain base's pair
(`(// c⏎a?.m().m().c(() => d)).z`), with a chain that breaks inside the pair —
the member-chain heuristic, a mid-chain `//`, or a function body.

- **tsv**: keeps the comment inside the pair and takes the expanded shell —
  the `//` on the `(` line, an own-line comment on its own line — and the chain
  inside it keeps its own continuation: the operand one indent in, its links
  two, the depths the comment-free base already prints at
  (`(⏎\ta⏎\t\t?.m()…⏎).z`). The two layouts differ only on the `(` line.

```
( // c1
	a
		?.m()
		.m()
		.c(() => d)
).z;
```

- **prettier**: also keeps the comment inside the pair, glued flush to the `(`,
  with the operand flush at the enclosing indent and the links one in
  (`(// c1⏎a⏎\t?.m()…).z;`) — so the divergence is the rendering alone; no
  comment leaves the pair.

Every position reaches the same shell: a member object, a `)!` seal, an
assignment value, a `return` argument, a callee through the chain, a comparison operand, an `if`
test, and the bare callee (c10). A trailing block in the same pair rides the
same shell beside the operand, fitting (c9) or breaking (c11), and a `return`
argument keeps the shell on the keyword line (c12). The
`unformatted_ours_glued_paren_shell` variant holds the authored compact
spellings (some through redundant parens), which tsv normalizes to this file in
one pass.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
