# union_intersection_paren_member_own_line_comment_prettier_divergence

Own-line **line** comment leading a retained parenthesized union member of an
intersection (`(a | b) & (⏎ // c⏎ a | b)`). The leading-own-line counterpart of
`union_intersection_retained_paren_line_comment` (which covers a *trailing* line
comment in an already-broken union): here the intersection would otherwise
collapse to one line, where a leading line comment has nowhere to sit.

**Prettier** collapses the intersection and relocates the comment out of the
parens onto its own line before `(`:

```
type T = (a | b) &
	// c
	(a | b);
```

**tsv** keeps the comment where the user wrote it — inside the parens, after
`(` — forcing the parenthesized member to break open:

```
type T =
	(a | b) &
	(
		// c
		a | b
	);
```

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it to the surrounding
intersection. tsv previously **dropped** this comment (content loss — it
collapsed the member inline, where a leading line comment cannot go); preserving
it is the fix.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
