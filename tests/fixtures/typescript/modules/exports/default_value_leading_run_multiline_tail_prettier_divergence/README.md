# default_value_leading_run_multiline_tail_prettier_divergence

A block comment trailing `export default` that the author broke after, then a **multi-line**
comment glued to the value (`export default /* x */⏎/* y⏎*/ aaa ? bbb : ccc`). The multi-line
comment cannot share the line of one that broke before it, so the break after `/* x */` is
forced in both formatters, and both print the comment outside the value's own group — the
conditional stays flat.

tsv (`input.svelte`) hangs the rest of the run one level under the keyword:

```ts
export default /* x */
	/* y
	 */ aaa ? bbb : ccc;
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
export default /* x */
/* y
 */ aaa ? bbb : ccc;
```

`unformatted_ours_paren_break.svelte` is the same run before a parenthesized value with a break
inside its parens (`/* y⏎*/ (⏎aaa ? bbb : ccc)`), which tsv lands on the same fixed point.

Prettier keeps the operand break here, and tsv hangs it indented, as at every keyword→value gap.
See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).
