# relational `<` followed by `>=` - Svelte divergence

This fixture pins the reading of a `>=` that follows a `<` operand: it is a relational
chain, `(a < b) >= d`, not a type-argument close plus an assignment.

## Why tsv Differs

**Acorn-typescript** (used by Svelte's parser) rescans the `>=` into `>` + `=`, closes a
type-argument list on the `>`, and then reads the `=` as an assignment — leaving an
instantiation expression on the left:

```typescript
const a1 = a < b >= d; // ❌ acorn-typescript: "Assigning to rvalue"
```

**tsc** does not: its `canFollowTypeArgumentsInExpression` rejects the type-argument
reading here, so the `<` stays relational and the `>=` stays a single operator. Prettier
(via typescript-estree) agrees — it reprints the unspaced `a<b>=d` as `a < b >= d`, which
it would not do for an instantiation. tsv matches tsc and prettier.

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc, and the practical accept test is whether prettier formats the
input — it does. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: parses both as relational chains (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)

Both operand forms are covered because they take different paths through the
type-argument lookahead: `a1` is a bare identifier, `a2` an indexed member access. The
non-`>=` members of this family — where acorn *does* accept, so `expected.json` exists —
live in the sibling
[relational_lt_vs_type_args](../relational_lt_vs_type_args/) fixture.
