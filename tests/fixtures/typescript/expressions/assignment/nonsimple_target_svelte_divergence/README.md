# non-simple assignment target - Svelte divergence

This fixture pins that a *non-simple* assignment target parses: a call (`foo() = bar`),
a compound assignment to one (`foo() += 1`), a literal (`1 >>= 2`), and `this`
(`this = x`).

## Why tsv Differs

"The left-hand side is not a valid assignment target" is a **static-semantic early
error**, not a syntax error. The grammar production is
`LeftHandSideExpression = AssignmentExpression`
([ecma262 §13.15](https://tc39.es/ecma262/#prod-AssignmentExpression)), which parses all
four fine; the "is it assignable?" refinement (`AssignmentTargetType`) is layered on top
as an early error. Per tsv's permissive-parser stance the parser defers it to the
diagnostics layer, so the formatter keeps formatting everything well-formed — and
prettier formats all four, which is the practical accept test.

**Acorn-typescript** (used by Svelte's parser) enforces the early error and rejects:

```typescript
foo() = bar; // ❌ acorn-typescript: "Assigning to rvalue"
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: parses all four (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats all four, and to exactly this input — so the fixture also pins
  formatting agreement, which is the claim a Rust test cannot make against a live oracle

**Contrast — the deferral does not reach a `for`-in/of head.** A no-declaration head is a
`LeftHandSideExpression` position that is *not* an assignment context, so a non-simple
target there stays a parse error in tsv, as it is in prettier. Those cases, and the
node-shape assertions for this family, are in `tests/nonsimple_assignment_target.rs`.
