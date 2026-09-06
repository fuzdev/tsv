# param_default_left_side_shell_comment_prettier_divergence

An own-line comment inside the grouping parens of a parameter default's **leftmost** node
(`x = (⏎// c⏎a as any).b`). The value's printer hoists the run ahead of its doc, and the
`=`→value seam gives the value the continuation indent it gives a line comment authored in
the `=`'s own gap
([param_default_line_comment](../param_default_line_comment_prettier_divergence/)) — the
one-pass fixed point: left at the parameter's level, the run rendered flush and the reparse
indented it.

tsv (`input.svelte`):

```ts
function fn1(
	x = // c
		(a as any).b
) {}
```

Prettier never converges on this authoring (`prettier_nonconvergent.txt`): for the member
value it oscillates between the comment on the `=` line with the value flush below and the
cast operand re-wrapped in an expanded shell that holds the comment
(`x = ( // c⏎\ta as any⏎).b`), flipping on every pass; the conditional value settles with
the comment floated past the value (`x = (a as any) ? b : c // c`). No prettier-anchored
claim is expressible.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
