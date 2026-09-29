# Binary operand paren at the chain's end, line comment

A `//` between a binary chain's **last** operand and the `)` of the pair its position
requires (`x - (y - z // c⏎)`). Covers an arithmetic, a logical and an exponent operand,
the same pair at a call argument and a parameter default, a comment the author gave its
own line, and a comment after the statement. The last cell is the control: a pair the
printer adds for clarity (`a || (b && c)`) has no `)` of its own in the source, so a
comment there belongs to the shell around the whole operand, and stays with it.

- **tsv**: keeps the comment inside the pair and opens it — the operand one indent in,
  the run behind it, the `)` back out on its own line — the expanded shell the unary
  comment-holder (`-(⏎\tx // c⏎)`) and the non-null operand (`(⏎\tx + y // c⏎)!`)
  already take:

```
const a =
	x -
	(
		y - z // c1
	);
```

- **prettier**: relocates the comment past the `)` and defers it to the end of the
  enclosing line (`(y - z); // c1`), where it merges with a comment already on that line
  (`(y - z); // c7 // c8` — one comment where there were two). Its second pass rejoins
  the chain its first pass broke (pinned in `audit_signature.txt`).

The comment is never the chain's to relocate: nothing after the last operand's `)`
belongs to the chain, so the deferred run would leave the construct it was written in.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
