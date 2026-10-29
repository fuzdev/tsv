# Return/throw/yield operand, block comment in a member gap

An own-line block comment in a member chain's object→property gap, in a `return` / `throw` /
`yield` argument. The chain prints that gap the way it prints it at every other value
position: before a plain member, and before a member the chain's head absorbs, the block is
glued between the object and the member (`return a /* c */.b;`, as `a /* c */.b;` prints it
at a statement — see
[pre_dot_own_line_block_comment](../../../expressions/member/pre_dot_own_line_block_comment_prettier_divergence/)).
Nothing then ends a line ahead of the argument, so no hanging pair is owed. The pair follows
what the gap PRINTS, read from the argument's built doc, and from the gap alone: a member-only
chain the printer breaks for width elsewhere still takes no pair. A gap whose comment keeps a
line of its own still takes it, since the reparse reads that comment as own-line and hangs the
argument: before a call the chain breaks before, a `//`, and a computed member.

The same read holds at a private member (`return a /* c */.#p;`), through an assignment
argument, where a `//` keeps its line and so the pair (`return (⏎	a⏎		// c⏎		.b = 1⏎);`),
and at a called member written on a multi-line block's closing line: where the chain keeps
that member glued behind the comments (`.c /* m⏎n */ /* c */.d(1)`), nothing ends a line
there and no pair is taken.

A block on the `)` line of a pair the chain keeps (an optional call's, holding a `//`) is
printed there, behind the `)`, with the chain breaking below it. It does not take a line of
its own either, so it takes no pair, though the reparse sees a newline after it.

Authored (`unformatted_ours_own_line.svelte`):

```ts
return a
/* c */
.b;
throw (a)
/* c */
?.b;
return (a?.() // c
) /* d */ .m();
return a
/* c */
.b();
```

tsv (`input.svelte`), in one pass:

```ts
return a /* c */.b;
throw a /* c */?.b;
return (
	a?.() // c
) /* d */
	.m();
return (
	a
		/* c */
		.b()
);
```

Prettier hoists the block above the argument, inside the hanging pair
(`return (⏎/* c */⏎a.b⏎);`), relocating it out of the gap it was written in; it hoists the `//`
the same way (`return (⏎// c⏎a.b⏎);`). At `yield`, which its pair never reaches, its output
loses the argument to ASI (`yield /* c */⏎a.b + 1;` reparses as `yield;`). For a member the
chain's head absorbs it reaches tsv's form on its second pass (`audit_signature_own_line.txt`),
and it keeps tsv's form of the member-only chain broken for width, though it hoists the block
out of the authored one. At the kept pair it moves both comments past the `;`
(`return (a?.()).m(); // c⏎/* d */`, `output_prettier.svelte`). At the called member it breaks
before `.d(1)`, a layout difference only.

The shell-run reading of the same rule, where a comment in a mid-chain shell prints in the
member gap after the head, is
[operand_paren_mid_chain_shell_comment](../operand_paren_mid_chain_shell_comment_prettier_divergence/).

Reason: comment position; the `yield` cell is a prettier bug (ASI). See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Return/throw/yield operand, block comment in a member gap) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
