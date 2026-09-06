# Yield hanging-comment paren retention, left-side operand

The [yield_hanging_comment_parens](../yield_hanging_comment_parens_prettier_divergence/)
rule reached through the operand's **left side**: the own-line comment sits inside the
paren shell of the operand's leftmost node — a member's object, a binary's left operand, a
conditional's test — rather than ahead of the whole operand. `yield` is a restricted
production (`yield [no LineTerminator here] AssignmentExpression`, ECMA-262 §15.5), so the
comment's break is legal only inside grouping parens; tsv walks the left side to find it,
retains the parens, and lifts the comment to lead the whole operand inside them — the same
walk prettier's `returnArgumentHasLeadingComment` runs for `return` / `throw`.

Authored:

```ts
yield (
	// c
	a as any
).b;
```

tsv (`input.svelte`):

```ts
yield (
	// c
	(a as any).b
);
```

Prettier strips the shell and trails the comment on the keyword, from either authoring:

```ts
yield // c
(a as any).b;
```

That output **no longer parses as the input did**: ASI ends the `yield` at the newline, the
operand becomes a separate expression statement, and the `yield` loses its argument —
prettier's own next pass writes the split out (`yield; // c`, pinned in
`audit_signature.txt`). Prettier's `returnArgumentHasLeadingComment` walk is scoped to
`ReturnStatement` / `ThrowStatement` and never reaches `YieldExpression`; tsv uses one
walk for all three restricted productions. `yield*` diverges in layout only — a bare
`yield*` is a syntax error, so ASI cannot silently split it.

Reason: content integrity. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Yield hanging comment, left-side operand) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
