# head_paren_shell_member_lifted_run_prettier_divergence

A union member's own **lifted trailing run** (`(B[] // c⏎// c2⏎) | c`) prints OUTSIDE the
per-member `align(2)` offset — prettier's `union-type.js` prints a comment-carrying member
as `printComments(align(2, typeDoc))`, comments outside the offset — **unless the member has
LEADING comments**, when the whole comment-wrapped doc goes inside instead.

This directory is the shape that makes the second half hard to see: the leading run is not in
the member's own paren shell but one link down, at the member's leading printed **EDGE**
(`((⏎// c⏎B)[] // t⏎)`), the same descent
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/)
documents. The shells strip, so that run prints ahead of the member's first code token just
as the member's own shell's would — and the reparse reads it out of the member's leading gap.
Read only at the shallow window, pass 1 answered "no leading comments" and pass 2 "yes", so
the lifted run changed sides of the offset between them.

## Both formatters end at `input`; prettier takes two passes

`input` is **prettier-stable**, and it is the form prettier's own chain from the paren
authoring settles on for every case but the last. What diverges is the normalization:
prettier's first pass keeps the lifted run flush under the `|`
(`prettier_intermediate_to_variant_paren_shell.svelte`), a form it does not itself hold, and
its second pass indents it into the offset. tsv reaches the fixed point in one pass. A
one-pass comparison against prettier therefore reads as a divergence here where there is
none.

## The one real divergence, and why the chain lands on a `variant`

Case **c13**: on a LATER member, an **own-line** run at the leading edge is relocated ABOVE
the `| ` — where the reparse binds it to the member above, so it leads nothing and the
lifted run stays flush. Prettier instead trails it on the previous member (`| z // c13`), the
sanctioned union leading-run position divergence its own chain converges to, which makes
prettier's second pass a `variant_trailing` rather than `input`. Both formatters hold that
form stable, so it is a dual-stable variant.

The split is the union gap's own (`Printer::union_gap_inline_run_start`): a comment the
author glued forward stays with the member and leads it (c10), an own-line one does not
(c13). Asked without that split, c13's two passes disagreed — the predicate contradicted its
own output one pass later, which is the same trap the union gap's arm already documents.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

**c1–c3** — a block run at the edge, under an array suffix, on the union's FIRST member. The
run leads the member, so its lifted trailing run indents into the offset.

**c4–c6** — the line-comment spelling of the same edge.

**c7–c9** — the same edge under an indexed access, the descent's second link.

**c10–c12** — a LATER member, whose glued (block) edge run still leads it.

**c13–c15** — a later member whose own-line edge run is relocated above the `|`. This is the
case the variant differs on.

## Files

`unformatted_ours_paren_shell.svelte` carries the paren authoring, which reaches `input`
under tsv only. `prettier_intermediate_to_variant_paren_shell.svelte` is prettier's unstable
first pass from it, and `variant_trailing.svelte` the form its second pass settles on.
