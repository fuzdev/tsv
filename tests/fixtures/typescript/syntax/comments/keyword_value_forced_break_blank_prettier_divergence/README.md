# keyword_value_forced_break_blank_prettier_divergence

A block comment trailing a keyword whose break is **forced** — by a line comment below it
(`new /* x */⏎⏎// y⏎C1()`) or by the comment itself spanning lines (`new /* x⏎y */⏎⏎C2()`) — with
an author blank after it. The blank survives with the break in both formatters, as it does
wherever a break is forced
([§Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).

tsv (`input.svelte`) hangs the tail one level under the keyword, blank kept:

```ts
new /* x */

	// y
	C1();
```

Prettier keeps the blank and leaves the tail flush (`output_prettier.svelte`):

```ts
new /* x */

// y
C1();
```

The cases cover `new`, `await`, a switch label's `case`, `satisfies`, `keyof`, the `typeof` type
query, `infer`, the function-type `=>` and `export default` — every keyword→value gap where
prettier leaves the comment in the gap, so the indent is the whole divergence. The gaps where
prettier moves the comment across the keyword are
[keyword_value_forced_break_blank_relocated](../keyword_value_forced_break_blank_relocated_prettier_divergence/).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
