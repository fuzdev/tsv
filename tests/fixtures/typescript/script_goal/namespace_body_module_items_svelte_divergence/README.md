# `export`/`import` in a namespace or ambient-module body at `Goal::Script` - Svelte divergence

This fixture pins that the module items a TypeScript namespace or ambient-module body
holds parse in a standalone *script* (the `goal` marker selects `Goal::Script`): an
`export` in a `namespace` body, in a namespace nested inside it and on a decorated class
there, in a `module M` body, in a `declare namespace` body, and an `import` beside an
`export` in a `declare module` body.

## Why tsv Differs

"`import`/`export` is only allowed in a module" is a rule about the file's **top level**.
A namespace or module body is not the top level — it is TypeScript's own module-item
context, at either goal. tsc decides whether a file is a module from its top-level
statements alone (`isFileProbablyExternalModule`: the first top-level `import`/`export`,
else an `import.meta` anywhere), so a file whose exports all sit inside namespaces is a
script to tsc, and parses with no diagnostic. Prettier formats every case to precisely
this input.

The same reading decides what the format fallback may infer: a script attempt refused at
a top-level `export` has proved the file a module, but an `export` inside a namespace body
proves nothing, so it is no goal gate.

**Acorn-typescript** rejects all of them:

```typescript
namespace N { export const a = 1; } // ❌ acorn: "'import' and 'export' may appear only with 'sourceType: module'"
```

`tsParseModuleBlock` reads the body through `parseStatement(null, true)`, commented
*"Inside of a module block is considered "top-level", meaning it can have imports and
exports"* — and base acorn's `sourceType` check fires there, before the TypeScript
plugin's own reading of the body is consulted. The same slip as the
[import-equals](../import_equals_svelte_divergence/) sibling. acorn-typescript is tsv's
AST-**shape** target but not its correctness oracle; for validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: parses every body's items (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats every case, and to exactly this input

A module item outside a namespace or module body still rejects at this goal — a top-level
`export`/`import`, and a decorated `export class` — which is what
`../module_only_constructs_invalid/` pins. `import.meta` is not a module item and stays a
syntax error in every position.
