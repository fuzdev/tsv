# operand_paren_leading_comment_conditional_sequence_prettier_divergence

An own-line comment inside the paren shell of a `return` / `throw` argument's **leftmost**
node, where that node heads a breakable group — a conditional's test or a sequence's first
expression. Both formatters settle on the same form: the hanging parens hold the argument
to the keyword's line (a break after `return` / `throw` is ASI), the comment leads the
argument inside them, and the group stays flat. The sibling
[operand_paren_leading_comment_left_side](../operand_paren_leading_comment_left_side/)
covers the left-side kinds prettier reaches in one pass.

Authored:

```ts
return (
	// c
	a as any
) ? b : c;
```

Both formatters, the fixed point (`input.svelte`):

```ts
return (
	// c
	(a as any) ? b : c
);
```

Prettier takes **two passes** to get there (`prettier_intermediate_inner_shell.svelte`): its
first pass attaches the comment to the test — the leftmost leaf, still inside the
conditional's group — so the hardline the comment carries breaks the conditional
(`(a as any)⏎? b⏎: c`); only once the comment has moved ahead of the whole argument does
the group fit flat. The same at a sequence head (`a,⏎b` → `a, b`). tsv lifts the comment
out of the left side when it wraps, so the authored form converges in one pass.

Not a placement difference — the fixed point is shared. See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
