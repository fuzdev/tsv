# compound assignment through a glued type-argument close in a call argument - Svelte divergence

`g(f<T>>= c)` assigns to a comparison: a `>` glued to `>=` is no type-argument close — the longest punctuator at that position is `>>=` — so the source reads `f < T >>= c`, not the instantiation `(f<T>) >= c`. As a call argument acorn-typescript accepts it, building an
`AssignmentExpression` (`(f < T) >>= c`) whose `left` is a `BinaryExpression` —
`expected_svelte.json` records that tree.

## Why tsv differs

An `AssignmentExpression`'s left must be a `LeftHandSideExpression`
([ecma262 §13.15](https://tc39.es/ecma262/#prod-AssignmentExpression)), and a binary
expression is not one: no production derives the form, so it is a *grammar* error, not the
`AssignmentTargetType` early error tsv defers for `foo() >>= c`. tsc's parser rejects it
(TS1005). acorn's acceptance is a gap in its own check, not a reading: it validates an
assignment's left only outside a paren or a call argument list. Inside one, the expression may
still turn out to be an arrow's parameters (`maybeInArrowParameters`), so acorn skips the
check a compound operator gets (`checkLValSimple`) and never runs it once the list is not
an arrow — and the window reaches everything nested in the list until a function body
resets it: an object value (`({ k: a + b >>= c })`), an array element, a template hole, a
spread, an arrow-parameter default, `async (…)` and `g?.(…)` alike. Outside the window —
a bare statement, `x = { k: a + b >>= c }`, an arrow or function body — acorn rejects the
same form (`Assigning to rvalue`), and it rejects the `=` operator inside the window too,
whose left it converts to a pattern. tsv rejects in every position, as it does the bare
form ([operator_target](../../operator_target/)); `tsv_rejects.txt` pins tsv's error.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../../docs/conformance_svelte.md#typescript-corrections).
