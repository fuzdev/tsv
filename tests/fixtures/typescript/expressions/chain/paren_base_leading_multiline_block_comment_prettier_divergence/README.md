# Parenthesized chain base, leading multi-line block comment

A multi-line block glued to the operand of a parenthesized member base
(`(/* c⏎ */ a ? b : c).d`, `(/* c⏎ */ a + b).d`, a cast) — the base's required
pair keeps the comment inside, on the `(` line, outside the operand's own group.

- **tsv**: a **ternary** base keeps its own layout around the block — flat when
  it fits (c2), and when it breaks the arms hang one indent in and the `)` drops
  to its own line (c1), the comment-free base's layout. A base that would
  **hang** inside its parens (a binary, a cast) hugs them
  instead: the block keeps the `(` line, the operand breaks one indent in, and
  `).d` closes on its last line (c3, c5) — the block's body cannot print flat,
  so the hang would open the parens around it even where the operand fits.

```
(/* c3
 */ aaaa… +
	bbbb… +
	cccc…).d;
```

- **prettier**: hoists the block out in front of the pair
  (`/* c3⏎ */ (⏎\taaaa… +⏎\t…⏎).d;`), re-binding it from the operand to the
  whole statement — the required-pair relocation tsv does not follow.

The fitting forms (c2, c4, c6) keep their operand flat in both formatters, apart
from the hoist. The `unformatted_ours_compact` variant holds the operands written on
one line, which tsv normalizes to this file in one pass.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation),
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Parenthesized binary member base) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
