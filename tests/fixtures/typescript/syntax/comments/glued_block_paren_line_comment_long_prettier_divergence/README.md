# glued_block_paren_line_comment_long_prettier_divergence

The print-width boundary of a kept glued-block pair at a declarator. The pair is laid out as a
JSDoc cast's, so it takes the cast's width rule: `… = /* c */ (` stays on the declarator's line
while it fits (`c1`, exactly 100 columns) and breaks after the `=` once it does not (`c2`, 101),
hanging the block and its pair under the declarator.

```ts
// tsv
const bbb… =
	/* c2 */ (
		// d2
		x
	);
```

Prettier strips the pair at both widths and moves the block ahead of the `=` (or leaves it
behind the `=` on the line it breaks onto), so the `//` lands between the block and the value.

## Variants

- `unformatted_ours_breaks_swapped.svelte` — `c1` written broken and `c2` unbroken: the break
  is the width's, not the author's, so both normalize to the input.
- `variant_breaks_swapped.svelte` — prettier's output for that variant, which both formatters
  keep: no pair is left in it, so nothing is glued to a `(`.

## Reason

**Comment position** — a glued block comment binds to the value it leads, and a stripped pair
must not let another comment land between them. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(a block comment glued to a grouping paren whose interior opens with a line comment).

## Related fixtures

- [glued_block_paren_line_comment](../glued_block_paren_line_comment_prettier_divergence/) — every expression position
