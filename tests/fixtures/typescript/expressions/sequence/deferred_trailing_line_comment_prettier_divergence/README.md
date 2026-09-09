# Sequence trailing line comment: convergence, not position

A `//` between a parenthesized sequence's last operand and its closing paren
(`(a, b // c⏎)`) has no landing inside the parens — it runs to end-of-line, so it
renders after whatever the enclosing construct still prints on that line. Both
formatters defer it past the `)` and reach the **same fixed point**; the comment's
position is not in dispute.

They differ on the **first** pass:

- **tsv**: one pass at every position. The deferred run's break is flush-scoped, so
  it forces only the group owning the next line opportunity *after* the comment. An
  enclosing construct that must break still does — the call argument keeps the
  comment on its own last line, the hanging `return` closes on its own line — while
  the operand run, with no line opportunity left before the `)`, stays flat.
- **prettier**: attaches the comment to the last **operand** where the sequence's
  parens are the last thing the statement prints, so the run splits one-per-line on
  pass 1 and collapses on pass 2 (`prettier_intermediate_bare`). Where a token of the
  enclosing expression follows the `)` — an `as`/`satisfies` keyword, a call's own
  `)` — it attaches to the **sequence** and the run stays flat, matching tsv on pass
  1 (`unformatted_wrapper` normalizes to `input` under both).

So the divergence is one line of `input`: `const x3 = (a, b); // c4`, whose authoring
prettier takes two passes to settle on. `unformatted_ours_bare` carries that
authoring.

Print width is unaffected: the operand separators are still break points, so a run
that does not fit still breaks one-per-line (see the `_long` sibling). A comment in
an operand **gap** is a different question and is unchanged — it flushes at the
comma, which is an obligated break (`x4`).

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Sequence trailing line-comment convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
