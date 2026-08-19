# intersection_trailing_object_head_paren_shell_line_comment_prettier_divergence

The leading-edge paren shell of
[head_paren_shell_required_pair_gap_line_comment](../head_paren_shell_required_pair_gap_line_comment_prettier_divergence/)
at an **optional tuple element** whose intersection ends in an **object literal** — the one
shape that answers the required pair with the *aligned trailing-object* layout
(`(A & {⏎↹a: 1;⏎  })`) rather than the shared open-shell one. The comment lands in exactly
the same place either way: the shell strips, so its `//` ends up in the required pair's own
`(` gap, which is where the reparse finds it and where the author would have written it
bare.

tsv, at c1:

```ts
type A = [
	( // c1
		B & {
			a: 1;
	  })?
];
```

`input.svelte` is the fixed point the **bare** authoring already settles on — `G` is that
authoring written directly, and `A` / `C` / `E` are the same comment one shell out, on the
`(` line (`A`), on its own line (`C`), and under an array suffix (`E`). All four must land
here.

## Reason: the aligned layout owns that `(`, so it owns the gap in front of it

The aligned builder prints its own `(`…`)` and emits its own `(`→intersection gap — which is
why a comment written *directly* there is kept and threaded in explicitly (the
`FirstIntersection` case of
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/)).
A **head shell**'s run belongs in that same gap once the shell strips, so the gap widens over
the shell's claim and the shell declines its own copy — the standing leading-edge seam
(`Printer::leading_edge_claim_and_start` / `with_claimed_shell_leading_run`), which every
other gap that can hold such a run already asks.

Without it the two spellings disagreed, in two different ways, and neither was a divergence:

- with the run **unclaimed**, the aligned layout declined outright and the pair fell through
  to the shared open-shell form (`( // c1⏎↹B & { a: 1 }⏎)?`), which is not tsv's own fixed
  point — the reparse finds the comment in the pair's own gap and takes the aligned layout
  after all, so pass 1 and pass 2 disagreed (**F1**);
- where the aligned layout was taken anyway (`E`, whose shell sits under an array suffix and
  so is invisible to the decline), the shell printed its own run against a `(` that had
  already been emitted — welded, with no normalizing space (`(// c3`) — and the reparse then
  printed the same comment through the gap's own emitter, with the space. Same F1, one
  emitter over.

Both spellings now render the one shape every retained shell does: the run on the `(` line
where the author glued it (or on its own line where they gave it one), the value indented
into the shell, the object's `})` closing at the member's `align(2)` offset. Which LINE the
comment takes stays the author's — the opening-delimiter rule, pinned across the family by
[paren_shell_glued_leading_line_comment](../paren_shell_glued_leading_line_comment_prettier_divergence/).

## Prettier

Prettier hoists the comment out in front of the pair (`[⏎↹// c1⏎↹(B & { … })?⏎]`), re-binding
it from the operand to the whole element — the divergence
[optional_element_paren_leading_line_comment](../tuple/optional_element_paren_leading_line_comment_prettier_divergence/)
already catalogs, and the same hoist the no-object sibling records. It does not reach that
form from the paren authorings in one pass: the chain is pinned by
`audit_signature_head_shell.txt`, whose first pass leaves `A` and `E` welded to the `(`
(`(// c1⏎↹↹B & { a: 1 })?`) before the second lifts them out.

## Cases

**c1** — the shell on the `(` line, the author's glue.
**c2** — the same shell with the comment on its own line, which it keeps.
**c3** — the shell one link further in, under an array suffix.
**c4** — the bare authoring, the comment written directly in the pair's own gap.

## Files

`unformatted_ours_head_shell.svelte` carries each case with its shell present and the
surrounding layout flattened; tsv normalizes it to `input.svelte`. The glue is authorship, so
each case keeps the line `input` gives it — re-spelling one would send it to the other fixed
point rather than back to this one.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
