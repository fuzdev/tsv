# type-assertion target in a for-of/for-in head - Svelte divergence

A type assertion around a simple target — the non-null postfix `a!` / `a.b!`, and the
binary-level `a as T` / `a satisfies T` / `<T>a` — used as a no-declaration `for`-of /
`for`-in head, bare and inside a destructuring pattern. The same targets outside a
for-head are ordinary and live in the sibling [cast_target](../cast_target/); this
fixture exists because the for-head position is where acorn-typescript rejects them.

## Why tsv Differs

**tsc accepts every form**, parser and checker alike: its `checkReferenceExpression`
skips assertions and parens together and asks only that an `Identifier` or an access
expression remain, the same test it applies to an assignment target — and tsc's own
suite asserts `for ((g satisfies number) of [10])` with a clean baseline
(`compiler/referenceSatisfiesExpression.ts`). ecma262 makes the for-head's target rule
a *Static Semantics: Early Error* (the LHS's `AssignmentTargetType` must be simple), the
class tsv defers per its permissive-parser stance, and tsv already accepts the same
assertions as assignment targets. prettier formats every form, printing the
binary-level assertions **without** parens in a for head (it strips `(a as T)` to
`a as T`, unlike the assignment target it keeps as `(a as T) = 1`), so the paren
spellings ride as `unformatted_*` variants.

**Acorn-typescript** (used by Svelte's parser) converts a for-head target in *binding*
mode, where every assertion kind raises "Unexpected type cast in parameter position":

```typescript
for (a! of arr) {
} // ❌ acorn-typescript: Unexpected type cast in parameter position
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

The line stays where tsc's checker draws it: a target that is not a simple reference
under its assertions — a call, `this` (TS2487), an optional chain (TS2780 / TS2781) — is
still rejected by both parsers and by prettier. Pinned here by `input_invalid_call_target.svelte`
and the two optional-chain files, `input_invalid_optional_chain_target.svelte`
(`for (a?.b of arr)`) and `input_invalid_optional_chain_nonnull_target.svelte`
(`for (a?.b! of arr)`, the same chain read through the `!`); acorn wraps a chain in a
`ChainExpression`, which no for-head `left` can carry, so it rejects both too. A
parenthesized chain is sealed (`for ((a?.b).c of arr)` is an ordinary member target),
and a JSDoc cast reads through to the same target under a nested assertion
(`for ([/** @type {T} */ (a as U)] of arr)`) — neither has a fixture form, since
prettier's TypeScript parser strips a JSDoc cast's parens in a for head, so both are
pinned by [`tests/nonsimple_assignment_target.rs`](../../../../../nonsimple_assignment_target.rs).

## Expected behavior

- **tsv parser**: parses every form, keeping the assertion node as the statement's
  `left` — a `TSNonNullExpression` / `TSAsExpression` / `TSSatisfiesExpression` /
  `TSTypeAssertion` over the `Identifier` / `MemberExpression`, the shape acorn emits for
  the same assertion as an assignment target (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats every form, and to exactly this input
