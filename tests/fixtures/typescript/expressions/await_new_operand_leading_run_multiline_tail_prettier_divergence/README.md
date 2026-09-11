# await_new_operand_leading_run_multiline_tail_prettier_divergence

`await`/`new`'s twin of
[default_value_leading_run_multiline_tail](../../modules/exports/default_value_leading_run_multiline_tail_prettier_divergence/):
a block comment trailing the keyword that the author broke after, then a **multi-line** comment
glued to the operand (`new /* x */⏎/* y⏎*/ C1()`). The multi-line comment cannot share the line of
one that broke before it, so the break after `/* x */` is forced in both formatters, and both
print it outside the operand's own group.

tsv (`input.svelte`) hangs the rest of the run and the whole tail one level under the keyword —
the indent a `//` in the same gap takes
([await_new_operand_line_comment](../await_new_operand_line_comment_prettier_divergence/)):

```ts
new /* x */
	/* y
	 */ C1();
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
new /* x */
/* y
 */ C1();
```

The cases cover an indentable and a preserved multi-line comment, an author blank between the two
comments (kept in both formatters), an `await` argument both bare and in the parens its
precedence needs, the run staying outside them, and a `new` expression as a call argument, whose
hang sits inside the argument list. `unformatted_ours_paren_break.svelte` spells each
operand in parens with a break inside them, which tsv lands on the same fixed point.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
