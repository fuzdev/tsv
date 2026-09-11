# keyword_value_forced_break_blank_relocated_prettier_divergence

The relocation gaps' half of
[keyword_value_forced_break_blank](../keyword_value_forced_break_blank_prettier_divergence/): a
block comment trailing a keyword or operator, a line comment below it forcing the break, and an
author blank between them (`x1 = a as /* x */⏎⏎// y⏎B`).

tsv (`input.svelte`) keeps both comments in the gap they were written in and the blank with the
forced break, the tail hung one level under the keyword:

```ts
x1 = a as /* x */

	// y
	B;
```

Prettier moves the block comment across the keyword, and the blank goes with the move
(`output_prettier.svelte`):

```ts
x1 = a /* x */ as // y
B;
```

Prettier's own form is not its fixed point — its second pass floats the line comment further,
past the statement (`x1 = a /* x */ as B; // y`), which `audit_signature.txt` pins.

The cases cover a class property's `=`, `as`, predicate `is`, a type parameter's `extends` and
`=`, and a conditional type's `extends`: the relocation gaps of
[§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation),
where tsv keeps the comment in place.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
