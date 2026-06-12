# union_intersection_retained_paren_line_comment_prettier_divergence

Trailing **line** comment inside a **retained** parenthesized union member — a
`(x | y)` whose parens are kept because it nests in an outer union, with a line
comment trailing the last inner member (`(a | b // c) | c`, `a | (b | c // c) | d`).

**Prettier**: relocates the comment out of the parens to trail the whole member,
keeping the inner union inline (`| (a | b) // c`):
```
type A1 =
	| (a | b) // c
	| c;
```

**tsv**: keeps the comment where the user wrote it, inside the parens trailing the
member. Because a line comment must end its line, the parenthesized union expands
to its broken form (one member per line) with `)` on its own line. The inner
content sits one level past the `| (` member offset and `)` at the offset — the
per-member offset every union member gets (rendered as whole tabs):
```
type A1 =
	| (
			| a
			| b // c
		)
	| c;
```

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it out. (Without this,
the comment was dropped entirely — a content-loss bug.) The block-comment sibling
`union_intersection_retained_paren_comment` keeps the member inline because a block
comment can stay inline (`(b | c /* c */)`); a line comment cannot, so it forces the
expanded layout here.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
