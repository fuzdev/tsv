# case_test_leading_run_multiline_tail_prettier_divergence

A block comment trailing `case` that the author broke after, before a multi-line comment glued
to the test (`case /* x */⏎/* y⏎*/ aaa ? bbb : ccc:`) or before a test that breaks on its own
(`case /* x */⏎{⏎aaa: 1⏎}:`). The break after `/* x */` is forced in both formatters, and both
print the multi-line comment outside the test's own group, so the conditional stays flat.

tsv (`input.svelte`) hangs the rest one level under `case`:

```ts
case /* x */
	/* y
	 */ aaa ? bbb : ccc:
```

Prettier leaves it at the case's own indent (`output_prettier.svelte`):

```ts
case /* x */
/* y
 */ aaa ? bbb : ccc:
```

`unformatted_ours_paren_break.svelte` spells the first test in parens with a break inside them,
and the second glued to the comment — tsv lands both on `input.svelte`.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(`export default`→value, `export =`→value and `case`→test, a forced break after the first
comment).
