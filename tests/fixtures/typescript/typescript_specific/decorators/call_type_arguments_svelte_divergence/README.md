# decorator call type arguments - Svelte divergence

This fixture pins explicit type arguments on a decorator's trailing call —
`@g<number>()` on an identifier head, `@a.b<T>()` on a member chain.

## Why tsv Differs

Type arguments are a TypeScript extension to the TC39 decorator grammar, which is
otherwise `DecoratorMemberExpression Arguments`. tsc accepts both forms with **no
diagnostic** (`tests/cases/conformance/esDecorators/esDecorators-decoratorExpression.2.ts`
is exactly these shapes and has no `.errors.txt` baseline), and prettier formats them —
which is tsv's accept test.

**Acorn-typescript** rejects:

```typescript
@g<number>() class C {} // ❌ acorn: "Leading decorators must be attached to a class declaration."
```

The message is acorn's generic fallback: having parsed `@g`, it finds a `<` its
decorator grammar has no arm for and abandons the decorator. acorn-typescript is tsv's
AST-**shape** target but not its correctness oracle; for validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

**A trailing call is required.** `@g<number>` with no `()` is a syntax error in tsc and
prettier alike ("Declaration expected"), because the `<` then reads as a relational
operator against the decorated class — so tsv rejects it too, exactly as it does for
`a?.<T>` with no call.

## Expected behavior

- **tsv parser**: parses both as a `CallExpression` carrying `typeArguments` (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats both, and to exactly this input — **unparenthesized**, since the
  callee is still the plain member chain the bare decorator grammar allows. tsv prints
  the same, so there is no formatting divergence here

Contrast the sibling [non_null_expression](../non_null_expression/), the other
tsc-accepted shape outside the TC39 decorator grammar — there prettier *does*
parenthesize (`@x!` → `@(x!)`), because a non-null assertion is not a decorator member
expression.
