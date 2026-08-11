# async_svelte_prettier_divergence

A top-level ambient **async** signature — `declare async function fn(): Promise<void>;`
— bare, generic, combined with a generator `*`, and `export declare`.

TypeScript bars it with **TS1040** ("'async' modifier cannot be used in an ambient
context"), and like the generator's TS1221 next door it is a **checker** grammar
error, not a parse error. tsc's parser builds one `FunctionDeclaration` carrying
`[DeclareKeyword, AsyncKeyword]` modifiers (and `asteriskToken` for the combined
form) and reports **no** `parseDiagnostics`. That places it in the ambient-context
early-error family tsv defers, so tsv parses it as a `TSDeclareFunction` with
`declare: true` and `async: true`.

## Why tsv differs from acorn

**Acorn-typescript rejects the bare form** (`Unexpected token`) — but **accepts**
`export declare async function`, emitting exactly the node tsv builds
(`declare: true, async: true`). That inconsistency is what settles the oracle
question: acorn is tsv's AST-**shape** target, not its correctness oracle, and a
verdict it reaches in one spelling and not the other is a slip rather than a
judgement. tsc accepts both, so tsv accepts both and matches acorn's *shape*.

Because acorn rejects the file as a whole, `expected_svelte.json` is the
parse-failure marker and `expected_ours.json` carries tsv's AST. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Why tsv differs from prettier

Prettier's `typescript` parser (typescript-estree) **rejects** it at parse time,
promoting the checker's grammar diagnostic to a parse failure:

```
'async' modifier cannot be used in an ambient context.
```

so there is no `output_prettier.*`. `prettier_rejects.txt` pins the error; rule F6
live-verifies that prettier still rejects with that message.

## The line-terminator boundary

`async` keeps its `[no LineTerminator here]` here as everywhere else, and the rule is
enforced **before** the ambient reading is committed to: `declare async⏎function
f(): void;` is not one ambient signature (tsc's modifier lookahead bails on the break
too, recovering into three statements; tsv rejects it, as it did before this form was
accepted at all). The `export declare` spelling reaches the same rule through its own
check — tsc rejects that one outright, TS1128.

## The rest of the family

The generator `*` alone — no `async` — is the sibling
[generator](../generator_prettier_divergence/), where acorn *accepts* and only
prettier rejects, so it is a prettier-only divergence. The plain ambient signature
both oracles agree on is [basic](../basic/).

See [conformance_prettier_ts.md](../../../../../../../docs/conformance_prettier_ts.md)
§Prettier rejects valid input.
