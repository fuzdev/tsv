# parenthesized assignment-tier assignment target - Svelte and prettier divergence

This fixture pins that a *parenthesized* conditional (`(a ? b : c) = 1`), arrow function
(`(() => b) = 1`, `(async () => b) = 1`) or `yield` (`(yield a) = 1`) parses as an
assignment target and keeps its parens — as the whole target, on the right of another
assignment (`x = (a ? b : c) = 1`), and as a destructuring default's target
(`[(a ? b : c) = 1] = x`). All three are alternatives of `AssignmentExpression` itself,
so bare, each one absorbs the `=` that follows it.

## Why tsv differs

**From Svelte (acorn-typescript).** The parenthesized form is a
`ParenthesizedExpression`, a `LeftHandSideExpression`, so the assignment production
derives it and only the `AssignmentTargetType` early error refuses it — the
static-semantic class tsv defers (see
[nonsimple_target](../nonsimple_target_svelte_divergence/)). tsc's parser accepts every
line; acorn enforces the early error (`Assigning to rvalue`). See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

**From prettier.** Prettier strips every target pair, and the stripped lines re-parse as
**different, valid programs**: `a ? b : c = 1` is `a ? b : (c = 1)`, `() => b = 1` is
`() => (b = 1)` and `yield a = 1` is `yield (a = 1)` — prettier's own second pass prints
exactly those (pinned by `audit_signature.txt`). tsv keeps the pair, the one spelling
that keeps the program. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript).

**Contrast — a parenthesized assignment target rejects.** `(a = b) = 1` is the one
assignment-tier target tsv refuses (an `input_invalid_*` file of
[nonsimple_target](../nonsimple_target_svelte_divergence/)), and not for its reprint:
target conversion turns the parenthesized `=` into an `AssignmentPattern`, a node acorn's
grammar has only as a pattern child, never as a whole target, so the tree holds no
assignment left to wrap. A conditional, an arrow or a `yield` is never converted — it
survives as itself, and the kept pair reprints it faithfully.

The operator-expression counterpart, whose stripped form does not parse at all, is
[operator_target_paren](../operator_target_paren_svelte_prettier_divergence/).
