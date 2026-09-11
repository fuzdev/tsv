# await_new_operand_leading_comment_breaking_value_prettier_divergence

`await`/`new`'s twin of
[default_value_leading_comment_breaking_value](../../modules/exports/default_value_leading_comment_breaking_value_prettier_divergence/):
a block comment trailing the keyword that the author broke after, before an operand that breaks on
its own (`await /* x */⏎fn1({⏎…⏎})`). The operand's own hard break forces the break after the
comment in both formatters.

tsv (`input.svelte`) hangs the operand one level under the keyword — for `new`, the whole tail,
callee and argument list alike:

```ts
await /* x */
	fn1({
		b: 1,
		c: 2
	});
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
await /* x */
fn1({
	b: 1,
	c: 2
});
```

`unformatted_ours_paren_blank.svelte` spells each `await` argument in parens holding a blank line;
the parens strip and the blank goes with them.

An operand that fits after an unforced break reflows onto the keyword's line instead
([await_new_operand_own_line_block_comment](../await_new_operand_own_line_block_comment_prettier_divergence/)):
nothing forces that break.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
