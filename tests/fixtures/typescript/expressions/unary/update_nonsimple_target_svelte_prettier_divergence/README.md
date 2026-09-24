# non-simple update operand - Svelte and prettier divergence

This fixture pins that an update expression parses over a *non-simple* operand. It covers
an optional chain (`a?.b++`, `++a?.b`, `a?.b!--`), a call (`foo()++`, `--foo()`), and a
literal (`1++`), in both the postfix and the prefix spelling. tsv parses each line as an
`UpdateExpression` whose `argument` is the operand acorn builds in a read position
(`ChainExpression`, `CallExpression`, `Literal`), and it reprints each line unchanged.

The operand rule is a **static-semantic early error**, not a syntax error. The grammar
production is `LeftHandSideExpression ++` / `++ UnaryExpression`, which parses every
operand. The "is it assignable?" refinement is layered on top ("It is an early Syntax
Error if the AssignmentTargetType of … is *invalid*",
[ecma262 §13.4.1](https://tc39.es/ecma262/#sec-update-expressions-static-semantics-early-errors)),
the same rule the non-simple `=` target carries. tsv defers it, and **tsc's parser
accepts every line** (its checker rejects them: TS2777 for the optional chain, TS2357 for
the rest).

## Why tsv differs from acorn

**Acorn-typescript enforces the early error** and rejects: `Optional chaining cannot
appear in left-hand side` for the chains, `Assigning to rvalue` for the call and the
literal. acorn is tsv's AST-**shape** target, not its correctness oracle, so tsv accepts
and matches acorn's *shape*. Because acorn rejects the file as a whole,
`expected_svelte.json` is the parse-failure marker and `expected_ours.json` carries
tsv's AST. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Why tsv differs from prettier

Prettier's `typescript` parser (typescript-estree) **rejects** every line at parse time,
promoting the early error to a parse failure:

```
Invalid left-hand side expression in unary operation
```

so there is no `output_prettier.*`. `prettier_rejects.txt` pins the error; rule F6
live-verifies that prettier still rejects with that message.

## The rest of the family

The same operands as an *assignment* target are the siblings
[nonsimple_target](../../assignment/nonsimple_target_svelte_divergence/) and
[optional_chain_target](../../assignment/optional_chain_target_svelte_divergence/), where
prettier formats every line. A paren after the chain seals it, so `(a?.b).c++` is an
ordinary member operand
([optional_chain_sealed_target](../../assignment/optional_chain_sealed_target/)).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Prettier rejects valid input.
