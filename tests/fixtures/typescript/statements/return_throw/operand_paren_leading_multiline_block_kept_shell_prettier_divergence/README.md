# Return/throw operand, kept shell, multi-line block comment

The [kept_shell](../operand_paren_leading_comment_kept_shell_prettier_divergence/) rule at
the **multi-line block** spelling. A `return` / `throw` / `yield` argument whose LEFTMOST
node is a cast's (`as` / `satisfies`) or a postfix update's (`++` / `--`) operand keeps
that operand's shell: the pair holds the comment, so nothing reaches the keyword's line and
no hanging pair is added around it. A block whose own text spans a line carries a
`LineTerminator` for ASI (ecma262 sec-comments), so the shell is load-bearing here exactly
as it is for a `//` — one rule, both spellings.

Authored (`unformatted_ours_glued.svelte`):

```ts
return (/* a
b */ a) as B;
```

tsv (`input.svelte`):

```ts
return (
	/* a
b */ a
) as B;
```

Prettier strips the shell and hoists the run onto the keyword's line:

```ts
return /* a
b */ a as B;
```

**Prettier has no fixed point on this input**, so no prettier-anchored claim file is
expressible (`prettier_nonconvergent.txt`, live-verified by rule F5): the `throw` case of
that output is a DEAD document — ASI leaves `throw` with no argument — and prettier cannot
re-read it ("A throw statement must throw an expression"). At `return` / `yield` the output
parses but is a different program, and prettier's own next pass writes the split out
(`return;` / `yield;` with the operand below).

`fn6` is the control: a single-line block spans no line, nothing is load-bearing, and the
redundant shell drops in both formatters.

Reason: content integrity. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Return/throw hanging comment, left-side operand) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
