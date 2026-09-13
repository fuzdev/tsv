# Return/throw operand, left-side shell, multi-line block comment

The [left_side](../operand_paren_leading_comment_left_side/) fixture's rule at the
**multi-line block** spelling: the comment sits inside the paren shell of the argument's
LEFTMOST node (`(/* a⏎b */ a).b`) and its own text spans a line, so it carries a
`LineTerminator` for ASI (ecma262 sec-comments) exactly as a `//` does. `return` / `throw`
are restricted productions (`return [no LineTerminator here] Expression`), so the hanging
parens are what keep the run off the keyword's line.

Authored (`unformatted_ours_left_shell.svelte`):

```ts
return (/* a
b */ a).b;
```

tsv (`input.svelte`):

```ts
return (
	/* a
b */ a.b
);
```

Prettier strips the shell and hoists the run ahead of the whole argument, from either
authoring:

```ts
return /* a
b */ a.b;
```

That output **no longer parses as the input did**: ASI ends the `return` at the comment's
line terminator, the argument becomes a separate expression statement, and the `return`
loses its argument — prettier's own next pass writes the split out (`return;`, pinned in
`audit_signature.txt`). At `throw` the same output does not parse at all. Prettier reads
the comment's interior terminator at the *keyword* gap
([operand_paren_leading_multiline_block_comment](../operand_paren_leading_multiline_block_comment/),
where the two agree), but `returnArgumentHasLeadingComment` runs
`hasLeadingOwnLineComment` alone down the left side, so its walk asks only whether a
newline *follows* the comment and never sees one *inside* it.

`fn10` rides `unformatted_ours_left_shell.svelte` in the already-normalized form rather than
the authored one, which is the one place this fixture cannot state its own rule: the output
prettier gives `throw (/* a⏎b */ a).b;` is a DEAD document, so the chain from that variant
cannot be walked at all and no marker describes it. The `throw` death itself is pinned next
door, by
[operand_paren_leading_multiline_block_kept_shell](../operand_paren_leading_multiline_block_kept_shell_prettier_divergence/)'s
`prettier_nonconvergent.txt`.

Every cell of `input.svelte` is a form **prettier reproduces byte for byte** — the
divergence is which authorings reach it, so the whole claim lives in the variant and the
fixture carries no `output_prettier.*`. The walk's stop condition (a leftmost node whose own
pair is REQUIRED, where a hanging pair would double the one that already holds the run) is
[operand_paren_required_inner_pair_multiline_block](../operand_paren_required_inner_pair_multiline_block_prettier_divergence/);
`fn9` is the binary argument, which reaches the same pair through its own conditional parens
either way.

Reason: content integrity. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Return/throw hanging comment, left-side operand) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
