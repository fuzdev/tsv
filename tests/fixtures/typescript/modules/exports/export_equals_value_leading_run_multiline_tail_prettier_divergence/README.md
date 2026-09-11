# export_equals_value_leading_run_multiline_tail_prettier_divergence

`export =`'s twin of
[default_value_leading_run_multiline_tail](../default_value_leading_run_multiline_tail_prettier_divergence/):
a block comment trailing `=` that the author broke after, then a **multi-line** comment glued
to the value (`export = /* x */⏎/* y⏎*/ aaa ? bbb : ccc`). The break after `/* x */` is forced
in both formatters, and both print the multi-line comment outside the value's own group.

tsv (`input.svelte`) hangs the rest one level under the keyword:

```ts
export = /* x */
	/* y
	 */ aaa ? bbb : ccc;
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
export = /* x */
/* y
 */ aaa ? bbb : ccc;
```

`unformatted_ours_paren_break.svelte` is the same run before a parenthesized value with a break
inside its parens, which tsv lands on the same fixed point.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment).
