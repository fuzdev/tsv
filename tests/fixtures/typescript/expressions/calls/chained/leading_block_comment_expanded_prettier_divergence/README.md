# leading_block_comment_expanded_prettier_divergence

A block comment leading a member chain call's first argument, in the layouts
where that argument list **expands**: a sole object/array literal wide enough to
break, and a function-composition list. The comment leads the argument in both
formatters — it does not ride the `(` line, and it breaks a wide literal out of
the hug instead of being carried into it.

Both formatters share one fixed point (`input.svelte`). The divergence is
convergence speed on the wide-object case: from a flat authoring prettier needs
two passes, tsv one.

## Pattern

Source (flat): `const a2 = obj.fn1().fn2(/* c */ { prop1: '…wide…' });`

- **Prettier pass 1** (unstable): the flat argument carries no forced break, so the
  chain's one-line measurement reads the whole flat content, overflows, and the
  chain expands.
- **Prettier pass 2** (stable): the now-multiline argument re-reads as
  authored-expanded, the fit measurement truncates at that break and sees only the
  chain head, which fits — the chain collapses back to flat with the argument
  broken out under it.

`unformatted_ours_flat` is that flat authoring; `unformatted_ours_paren` writes
the comment ahead of the `(` instead, which both formatters move to the
argument's side of it. tsv normalizes each in one pass.

## Reason

Prettier bug (non-idempotent): from the flat authoring prettier's first pass is
not a fixed point. tsv prints prettier's own settled form in one pass — a
convergence-speed divergence with a single authoring-independent fixed point.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
for the gap's entry, and
[conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript)
for the convergence rule it inherits (Member-chain wide-last-argument hug convergence).
