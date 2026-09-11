# keyword_value_leading_comment_breaking_value_prettier_divergence

The type keyword→value gaps' twin of
[await_new_operand_leading_comment_breaking_value](../../expressions/await_new_operand_leading_comment_breaking_value_prettier_divergence/):
a block comment trailing the keyword that the author broke after, before a type that breaks on its
own (`keyof /* x */⏎{⏎aaa: 1;⏎}`). The type's own hard break forces the break after the comment in
both formatters.

tsv (`input.svelte`) hangs the type one level under the keyword:

```ts
type T1 = keyof /* x */
	{
		aaa: 1;
	};
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
type T1 = keyof /* x */
{
	aaa: 1;
};
```

The cases cover `keyof`, the function-type `=>` and `readonly`.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
