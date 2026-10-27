# Return/throw operand, mid-chain shell comment

A comment inside the paren shell of a member chain's link **below its head**, in a
`return` / `throw` argument. The chain prints the shell's comments in the member gap after
its head, where they were written, so the hanging parens follow what that gap prints. One
glued to its `(` and first in the run trails the head on the keyword's line and takes no
pair; one on a line of its own takes it, as does one glued to its own `(` below a `//` the
run already printed, or below a line the source opened before that `(`. The reparse reads
each as an own-line comment ahead of `.b`, and hangs the argument, so the first pass hangs
it too.

Authored (`unformatted_ours_shell.svelte`):

```ts
return ((// d
(a).b()).c.m());
return (
	// c
	a.b
).m(1);
return (// d
	(// e
	a.b)).m();
return (/* d */
	(// e
	a.b)).m();
return a.b.c /*
*/ /* e */.m();
```

tsv (`input.svelte`), in one pass:

```ts
return a // d
	.b()
	.c.m();
return (
	a
		// c
		.b.m(1)
);
return (
	a // d
		// e
		.b.m()
);
return (
	a /* d */
		// e
		.b.m()
);
return (
	a.b.c /*
		 */ /* e */
		.m()
);
```

Prettier moves each comment past `.b` (`return a.b // c⏎.m(1)`). It takes two passes from
`input.svelte` (`output_prettier.svelte` glues `a// c`, `audit_signature.txt` pins the rest)
and three from the authored forms (`audit_signature_shell.txt`). At the nested shells it
**merges** the two comments into one, in reverse (`a.b // e // d`), so `// d` survives only
as the text of `// e`.

A comment on a multi-line block's closing line in a called member's object→property gap
(`return a.b.c /*⏎*/ /* e */.m()`) takes the pair as well: the block forces the chain open,
the call's member starts a line of its own, and the comment then ends its line, which the
reparse reads as own-line and hangs. Prettier keeps `return a.b.c /*⏎ */ /* e */⏎.m();` stable
with no pair — a divergence in the layout alone, since that output reparses as authored.

The leftmost-shell reading of the same rule, where the run is hoisted ahead of the whole
argument, is
[operand_paren_leading_multiline_block_left_side](../operand_paren_leading_multiline_block_left_side_prettier_divergence/).

Reason: content integrity and comment position. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Return/throw hanging comment, mid-chain shell) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
