# keyword_value_leading_run_multiline_tail_prettier_divergence

The type keyword→value gaps' twin of
[await_new_operand_leading_run_multiline_tail](../../expressions/await_new_operand_leading_run_multiline_tail_prettier_divergence/):
a block comment trailing the keyword that the author broke after, then a **multi-line** comment
glued to the type (`keyof /* x */⏎/* y⏎*/ B`). The break after `/* x */` is forced in both
formatters.

tsv (`input.svelte`) hangs the rest of the run and the type one level under the keyword — the
indent a `//` in the same gap takes
([type_operator_keyword_line_comment](../type_operator_keyword_line_comment_prettier_divergence/)):

```ts
type T3 = keyof /* x */
	/* y
	 */ B;
```

Prettier leaves it flush (`output_prettier.svelte`):

```ts
type T3 = keyof /* x */
/* y
 */ B;
```

The cases cover the function-type and constructor-type `=>`, `keyof` (with a preserved multi-line
comment and an author blank, kept in both formatters), `readonly`, the `typeof` type query,
`infer`, and `keyof` as a union member, whose hang sits under the member — tsv's tab indent where
prettier aligns two spaces past the `| `. A union return type keeps its own break-after-arrow layout and is not a case here.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
