# operand_paren_leading_comment_kept_shell_prettier_divergence

A `return` / `throw` / `yield` argument whose **leftmost** node is a cast's (`as` /
`satisfies`) or a postfix update's (`++` / `--`) operand, with an own-line comment inside
that operand's grouping parens. Those two positions retain a shell whose leading gap holds
a comment — the cataloged "a leading comment alone still keeps the shell" rule of
[as_satisfies_operand_line_comment](../../../expressions/as_satisfies_operand_line_comment_prettier_divergence/)
and
[update_postfix_paren_line_comment](../../../expressions/unary/update_postfix_paren_line_comment_prettier_divergence/)
— so the comment never leaves the pair, nothing reaches the keyword's line, and the
restricted production needs no hanging pair of its own. The left-side walk that adds one
(`operand_paren_leading_comment_left_side`) stops at these operands on purpose:
descending would wrap a second pair around the one that survives.

tsv (`input.svelte`):

```ts
return (
	// c
	a
) satisfies B;
```

Prettier strips the operand's shell, hoists the comment ahead of the whole argument, and
wraps the argument in its own hanging pair (`output_prettier.svelte`):

```ts
return (
	// c
	a satisfies B
);
```

Both are ASI-safe and both are fixed points; the difference is which pair holds the
comment. The shell-retention rule is the sanctioned one, stated at the cast and update
entries in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
