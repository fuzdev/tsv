# left_spine_retained_paren_prettier_ignore_interior_prettier_divergence

An own-line directive the author wrote inside a paren pair the printer **RETAINS** ahead of a
construct's leftmost operand — the counterpart of the erased-paren case
([left_spine_paren_prettier_ignore_interior](../left_spine_paren_prettier_ignore_interior_prettier_divergence/),
where the pair strips and the run hoists ahead of the construct). Here the pair survives, so
the directive keeps its own line *inside* it and the reparse reads it in the very same place:

```ts
const a = (
	// prettier-ignore
	{x:   1}
) as T;
```

Each cell retains its pair for a reason of its own, not the directive's: the `as` (resp.
`satisfies`) operand→keyword gap is ASI-sensitive, so a run that ends a line holds the pair
open, and the non-null operand needs precedence parens around `d ?? e`. The operand inside
freezes either way.

## Why tsv differs

Prettier strips the pair, relocates the directive to trail the `=` and freezes from there
(`output_prettier.svelte`). That placement is **inert** under tsv's classification, and
prettier's own second pass proves the point: it reformats the operand the directive froze on
all three hosts (pinned by `audit_signature.txt`). Keeping the author's line inside the pair
tsv was going to print anyway is the placement that holds the freeze across a second pass.

## Reason

◆comment_preservation ◆prettier_bug — sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads, and on sequence operands*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
