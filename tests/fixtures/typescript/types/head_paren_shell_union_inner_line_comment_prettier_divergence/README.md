# head_paren_shell_union_inner_line_comment_prettier_divergence

A redundant paren **layer** around an intersection's first member, where that member is a
parenthesized **union** (`((// c⏎ B | C)) & D`), swept across the positions the intersection
can sit in. It is the position sweep for the rule
[intersection_retained_paren_first_member_leading_line_comment](../intersection_retained_paren_first_member_leading_line_comment_prettier_divergence/)
states: **which pair survives is the member-parens rule, not the layer count the author
wrote.** The union's pair is required as an `&` operand, so exactly one pair is emitted
however many layers were nested, and the run renders inside it.

tsv, at c1:

```ts
type A = [
	(( // c1
			B | C
		) &
			D)?
];
```

## Reason: a paren layer is not authorship

Asked of the paren's **direct** child instead, the nested spelling matched nothing here, so
no route claimed the run: the member's own builder reached
`build_type_doc_maybe_parens_impl`'s union arm with `ShellLeadingRun::Upstream` — a licence
granted on an upstream emitter that only an intersection's *first* member has, and one the
first-member hoist then declined to be, because it too was asked of the direct child. What
came out was the run welded to a `(` that never opened (`(// c1⏎↹↹(B | C) & D)?` — no
normalizing space, the operand un-indented, the `)` on its tail), the third form
`build_open_required_paren_doc` exists to prevent. The reparse then finds the comment in the
pair's own gap and prints the opened shape, so the two passes disagreed — an **F1**
violation, not a divergence, at all six positions off one predicate. (At a *later*
intersection member the same predicate left the run with no emitter at all and the comment
was outright **DROPPED**; that is the same rule read one member over, in
[intersection_retained_paren_first_member_leading_line_comment](../intersection_retained_paren_first_member_leading_line_comment_prettier_divergence/).)
Prettier converges its own spellings too — `((// c⏎ B | C)) & D`, `(// c⏎ (B | C)) & D` and
`(// c⏎ B | C) & D` all give it one answer — which is the argument that the layer carries no
signal to preserve.

The positions matter because each reaches the member through a different builder: an
optional tuple element's required pair (c1), a union member (c2), an array element (c3), an
indexed access's object (c4), a conditional's check type (c5), and the plain alias value as
the control (c6). All six now land on one shape.

## Prettier

Prettier hoists the comment out in front of the pair, re-binding it from the operand to the
whole element — the relocation the sibling fixtures already catalog. Its own output is not
idempotent at the array element and the indexed access, so its chain is pinned by
`audit_signature.txt`.

## Files

`unformatted_ours_union_layer.svelte` writes each case with the redundant layer present;
tsv normalizes it to `input.svelte`.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
