# optional-chain assignment target - Svelte divergence

This fixture pins that an optional chain parses as an assignment target: with `=`
(`a?.b = c`), with a compound operator (`a?.[0].c += d`), with a logical assignment
(`a?.b ??= c`), as a destructuring element (`[a?.b, ...a?.c] = x`, `({ x: a?.b } = y)`,
`({ ...a?.b } = y)`), as the target of a destructuring default (`[a?.b = 1] = x`), and
with a non-null assertion (`a?.b! = c`, `a?.b!.c = d`). The wire carries each target as
the `ChainExpression` acorn builds for the same chain in a read position. The
`unformatted_paren_target` variant wraps the chains in redundant parens
(`(a?.b) = c`), and both formatters strip them.

## Why tsv Differs

An optional chain is not a valid assignment target, but that is a **static-semantic
early error**, not a syntax error. ecma262 gives `LeftHandSideExpression :
OptionalExpression` an `AssignmentTargetType` of *invalid*
([ecma262 §13.15.1](https://tc39.es/ecma262/#sec-assignment-operators-static-semantics-early-errors)),
and the assignment and destructuring early errors reject on that (a logical assignment
asks for a *simple* target, which the chain is not either). tsv's parser defers
the early error like every other non-simple target. tsc's parser accepts every line, and
the rejection comes from its checker (TS2779, and TS2778 for the object-rest target). prettier formats every line.

**Acorn-typescript** (used by Svelte's parser) enforces the early error and rejects:

```typescript
a?.b = c; // ❌ acorn-typescript: "Optional chaining cannot appear in left-hand side"
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: parses every line (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats every line, and to exactly this input

**Contrast.** A paren *after* the chain seals it. `(a?.b).c = d` is an ordinary member
target that acorn accepts too, so it lives in the plain sibling
[optional_chain_sealed_target](../optional_chain_sealed_target/). The deferral does not
reach a no-declaration `for`-in/of head (`for (a?.b of xs)`), which rejects on both sides
([cast_target_for_head](../cast_target_for_head_svelte_divergence/)).
