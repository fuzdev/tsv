# generator_prettier_divergence

A top-level ambient generator signature — `declare function* gen(): Iterator<number>;`
— in every spelling the `declare` head takes: bare, generic, overloaded, and
`export declare`.

TypeScript's rule against it is **TS1221** ("Generators are not allowed in an ambient
context"), and it is a **checker** grammar error, not a parse error:
`checkGrammarFunctionLikeDeclaration` raises it via `grammarErrorOnNode` in
`checker.ts`, so `ts.createSourceFile(…).parseDiagnostics` is empty on every case
here. That makes it a static-semantic early error of exactly the ambient-context
family tsv defers — the same class as the `declare` member bodies, initializers and
decorators named in
[conformance_svelte.md §TypeScript Corrections](../../../../../../../docs/conformance_svelte.md#typescript-corrections)
— so the parser accepts it and correctness stays tsc's job.

**Acorn-typescript** (used by Svelte's parser) also accepts, so this is not a Svelte
divergence: `expected.json` is the ordinary canonical AST, with `generator: true` on
each `TSDeclareFunction`.

Prettier's `typescript` parser (typescript-estree) **rejects** it at parse time,
promoting the checker's grammar diagnostic to a parse failure:

```
Generators are not allowed in an ambient context.
```

so prettier cannot serve as a formatting oracle here — there is no
`output_prettier.*`. `prettier_rejects.txt` pins the error; rule F6 live-verifies
that prettier still rejects the input with that message, failing loudly if prettier
is ever relaxed or the error morphs.

See [conformance_prettier_ts.md](../../../../../../../docs/conformance_prettier_ts.md)
§Prettier rejects valid input.

## The rest of the family

The `*` is the only thing at issue — `declare function gen(): void;` is ordinary
[basic](../basic/), and the overload spelling without it is [overloads](../overloads/).
Two sibling positions carry the same construct under different oracle verdicts, and
each has its own fixture:

- A **bodiless** `function*` signature nested in a `declare namespace` / `declare
  module` / `declare global`, or in a plain `namespace` — prettier rejects those with
  a *different* message ("A function signature cannot be declared as a generator"),
  pinned by
  [namespace/generator_signature](../../../../declarations/namespace/generator_signature_prettier_divergence/)
- A `declare class` generator **method** — prettier accepts that one, so it is an
  ordinary fixture, [class/generator_members](../../class/generator_members/)

A `.d.ts`-shaped input adds nothing to pin: tsv has no ambient-file mode, so a
`.d.ts` parses exactly as a `.ts` and the shape is this fixture's. tsc's own corpus
makes the same pair — `generatorInAmbientContext2.ts` and
`generatorInAmbientContext4.d.ts` are byte-identical but for the extension, and draw
the identical TS1221 baseline.
