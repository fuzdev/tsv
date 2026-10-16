# bare type-assertion assignment target - Svelte divergence

`x as T = 1` assigns to an unparenthesized `as` expression. acorn-typescript accepts it,
building an `AssignmentExpression` whose `left` is the `TSAsExpression` —
`expected_svelte.json` records that tree.

## Why tsv differs

An `AssignmentExpression`'s left must be a `LeftHandSideExpression`
([ecma262 §13.15](https://tc39.es/ecma262/#prod-AssignmentExpression)), and a bare type
assertion is not one: no production derives the form, so it is a *grammar* error, not the
`AssignmentTargetType` early error tsv defers for `foo() = bar`. tsc's parser rejects it
(TS1005 `';' expected.`: `parseAssignmentExpressionOrHigher` reads an assignment operator
only after a `LeftHandSideExpression`, and an `as`, `satisfies` or `<T>` assertion is none),
and so does prettier. acorn's target check admits the assertion kinds over an identifier or
a member. Accepting it would also leave the formatter only a repair: the one spelling every
parser reads, `(x as T) = 1`, turns a program tsc rejects into one it accepts, and is no
reprint of the input.

tsv rejects every bare assertion target — `as`, `satisfies` and `<T>`, under every
assignment operator — and keeps the parenthesized spelling, a `ParenthesizedExpression` that
the production derives ([cast_target](../cast_target/)); `tsv_rejects.txt` pins tsv's error.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
