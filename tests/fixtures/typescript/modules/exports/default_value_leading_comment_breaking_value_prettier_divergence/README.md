# default_value_leading_comment_breaking_value_prettier_divergence

A block comment trailing `export default` that the author broke after, before a value that
breaks on its own (`export default /* x */⏎{⏎aaa: 1⏎}`). The value's own hard break forces the
break after the comment in both formatters, as at every other value position.

tsv (`input.svelte`) hangs the value one level under the keyword:

```ts
export default /* x */
	{
		aaa: 1
	};
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
export default /* x */
{
	aaa: 1
};
```

A value that fits reflows onto the keyword's line instead
([default_value_same_line_comment](../default_value_same_line_comment_prettier_divergence/)):
nothing forces that break.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).
