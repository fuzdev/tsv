# parenthesized function-expression assignment target - Svelte and prettier divergence

This fixture pins that a *parenthesized* function expression (`(function () {}) = 1`,
`(async function () {}) = 1`) parses as an assignment target and keeps its parens — at
statement start, on the right of another assignment, as a call argument, and under a
compound operator (`x = (function () {}) += 1`).

## Why tsv differs

**From Svelte (acorn-typescript).** A function expression is a `PrimaryExpression`, so
the assignment production derives it and only the `AssignmentTargetType` early error
refuses it — the static-semantic class tsv defers (see
[nonsimple_target](../nonsimple_target_svelte_divergence/)). tsc's parser accepts every
parenthesized line; acorn enforces the early error (`Assigning to rvalue`). See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

**From prettier.** At statement start both formatters keep the pair (a leading
`function` would begin a declaration). Everywhere else prettier strips it, and tsc's
parser does not read an `=` after a function expression's body: `x = function () {} = 1`
fails with TS2809 (`Declaration or statement expected. This '=' follows a block of
statements…`), so prettier's output does not re-parse under its own TypeScript parser.
tsv keeps the pair. It keeps it under a compound operator too, though tsc and prettier
both read the bare `x = function () {} += 1`: the rule is one rule by the target's kind,
as the type-assertion pair is (`(a as T) += 1`), and it keeps the author's pair rather
than strip it where one operator happens to allow that — and the
`unformatted_ours_bare_compound` variant pins tsv repairing the bare compound spelling to
it (prettier leaves that one bare, as in `output_prettier.svelte`). See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript).

A class expression needs no pair: tsc reads `x = class {} = 1` as the same program.
