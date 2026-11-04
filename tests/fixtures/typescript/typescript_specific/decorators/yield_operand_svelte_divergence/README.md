# Parser correction: a decorated class as the operand of `yield`

`yield @dec class {}` is grammatical. The decorators proposal writes
`ClassExpression : DecoratorList? class BindingIdentifier? ClassTail`, so a decorated
class is an `AssignmentExpression` like any other and `YieldExpression : yield
AssignmentExpression` takes it. tsc reads it that way (`AtToken` is in its
`isStartOfExpression`), and prettier formats it.

acorn-typescript **rejects** it (`Unexpected token` at the `@`): `yield` takes an operand
only when the next token starts an expression, and acorn's `@` token does not say it does.
It accepts the `@` after `await` (`await @dec class {}`), after `yield*`
(`yield* @dec class {}`) and after a prefix operator (`!@dec class {}`). tsv parses every
shape here (`expected_ours.json`; the canonical parser errors → `expected_svelte.json`),
and the tree is the one acorn builds for the parenthesized spelling, positions aside.

prettier formats the input identically (parse correction only — no
`output_prettier.svelte`). The `e` line is an assignment operand, which both tools print in
its own pair (`yield (@dec⏎class {}.x = 1)`); acorn reads that line, and the rest of the
input keeps the shape. `unformatted_compact` writes every line inline, as the author would:
both tools break only at the decorator, never between `yield` and `@`. The authored-pair
spelling, which acorn accepts, is
[yield_operand_paren](../yield_operand_paren_prettier_divergence/).

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
