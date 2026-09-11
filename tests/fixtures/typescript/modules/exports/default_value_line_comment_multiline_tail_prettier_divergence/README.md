# default_value_line_comment_multiline_tail_prettier_divergence

A line comment trailing `export default`, then a **multi-line** comment glued to the value
(`export default // x⏎/* y⏎*/ aaa ? bbb : ccc`). Both formatters keep the comment on its own
line and print it outside the value's own group — the conditional stays flat.

tsv (`input.svelte`) hangs the value one level under the keyword, the uniform forced-continuation
indent:

```ts
export default // x
	/* y
	 */ aaa ? bbb : ccc;
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
export default // x
/* y
 */ aaa ? bbb : ccc;
```

`unformatted_ours_paren_break.svelte` is the same run before a parenthesized value with a break
inside its parens (`/* y⏎*/ (⏎aaa ? bbb : ccc)`), which tsv lands on the same fixed point.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
