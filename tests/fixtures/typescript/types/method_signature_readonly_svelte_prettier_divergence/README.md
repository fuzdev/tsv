# method_signature_readonly_svelte_prettier_divergence

A `readonly` modifier on a **method signature** — in an interface and in a type literal,
plain, optional, generic, computed-keyed and string-keyed.

TypeScript bars it with **TS1024** ("'readonly' modifier can only appear on a property
declaration or index signature"), a **checker** grammar error, not a parse error: tsc's
parser builds a `MethodSignature` carrying a `ReadonlyKeyword` modifier and reports no
`parseDiagnostics`. That places it in the modifier early-error family tsv defers, so tsv
parses it and prints the modifier where the author wrote it — the input is its own fixed
point, with no token lost.

## Why tsv differs from acorn

Acorn-typescript rejects it in the parser (`ReadonlyForMethodSignature`, the same TS1024
wording), so `expected_svelte.json` is the parse-failure marker and `expected_ours.json`
carries tsv's AST. See
[conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections).

## Why tsv differs from prettier

Prettier's `typescript` parser (typescript-estree) rejects it at parse time, promoting the
checker's diagnostic to a parse failure:

```
'readonly' modifier can only appear on a property declaration or index signature.
```

so there is no `output_prettier.*`. `prettier_rejects.txt` pins the error; rule F6
live-verifies that prettier still rejects with that message.

## The boundary

`readonly` is a modifier only when a property name follows on the same line — tsc's
`nextTokenIsOnSameLineAndCanFollowModifier`. Followed by `(` or `<` it is the member's own
name (`readonly(): void`, `readonly<T>(): void`), a method both oracles accept.

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input).
