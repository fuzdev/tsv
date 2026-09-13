# return_type_function_multiline_block_comment_prettier_divergence

A **multiline** block comment trailing the function type inside an arrow's
disambiguating return-type pair (`(x: T): ((y: T) => T /* m1⏎m2 */) =>`), the
multiline face of
[return_type_function_comment](../return_type_function_comment_prettier_divergence/).

**tsv**: keeps the comment inside the pair and opens the pair, the form a trailing `//`
already takes there:

```
const a =
	(
		x: T
	): (
		(y: T) => T /* m1
m2 */
	) =>
	(y) =>
		y;
```

**Prettier**: hoists the comment out past the `)`:

```
const a =
	(
		x: T
	): ((y: T) => T) /* m1
m2 */ =>
	(y) =>
		y;
```

## Reason

An arrow's `=>` is `[no LineTerminator here]` (ECMA-262 `ArrowFunction`), and the
comment's interior newline is a line terminator: prettier's form is a syntax error to
acorn and to tsv (prettier's own TypeScript parser tolerates it, so prettier reaches a
second-pass fixed point on it — pinned in `audit_signature.txt` — but the output is not
the program the input was). The pair is what holds the author's break legal, so it has
to survive with the comment inside it — the same rule the sibling records for a `//`,
now keyed on any comment that spans a line.

Two authorings of the shell's **leading** gap ride the same pair: a `//` glued to the
`(` keeps that line (`( // c1`, the opening-delimiter rule), and an own-line comment
keeps its own line inside the pair. Prettier strips the shell in both, hangs the leading
comment after the `:`, and emits the trailing block outside the pair it re-synthesizes.
Before this rule the `:`→type hang stripped the shell in those two authorings and lifted
the trailing block out past the synthesized pair, the same dead form.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
