# Consecutive line comments before a ternary operator

Two or more line comments in the gap between a ternary operand and the
following operator — between the **test** and `?`, or between the
**consequent** and `:`.

- **tsv**: the first comment trails its operand (`cond // c1`); each subsequent
  comment drops to its own line, aligned with the operator it precedes
  (`// c2` on the line above `?`). Position preserved, none merged.
- **prettier**: relocates every comment after the first to the *other* side of
  the operator (`? // c2`), moving it from "before `?`" to "after `?`" — a
  change of syntactic association.

A single trailing comment (`cond // c1 ? …`) is a **match** and stays a plain
fixture (see `basic_comment`, `test_trailing_long_comment`). The divergence
only appears once a second comment forces the question of where it lands.

This is the before-operator mirror of `consecutive_branch_comment` (which keeps
consecutive comments *after* `?`/`:` in place — a match) and a direct
application of the [comment-position
philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
principle 1: "comments between an operator and its operand stay there." It is
also the ternary face of prettier's information-destructive comment motion
documented for property signatures (two leading line comments `merge and
reverse`); tsv preserves each comment as its own node.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
