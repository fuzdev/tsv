# Import Records

`parse_imports` answers which modules a source requests: one record per module request — a
static `import`, a re-export, a dynamic `import()`, a TypeScript `import x = require('y')` —
saying where it is, what it names (the specifier decoded), and whether it asks for types alone,
plus whether the source uses module syntax at all. It is a full parse, not a scan: the records
come from tsv's own parser, so a regex literal, an `import` used as a key, a template and an
escaped specifier are read as the grammar reads them. The implementation is `tsv_ts`'s
`crates/tsv_ts/src/imports.rs`, the records collected by the type strip's walk
([architecture.md §One Walk, Two Products](./architecture.md#one-walk-two-products-the-strip-and-the-import-records));
the fixtures that define them are [tests/fixtures_imports/](../tests/fixtures_imports/README.md).

## Surfaces

- **Rust**: `tsv_ts::parse_imports(source) -> Result<Imports, Error>`, the unset goal, and
  `tsv_ts::parse_imports_with_goal(source, goal: Goal)`, exact; `tsv_svelte::parse_imports(source)`
  for a component. Each parses into a fresh arena of its own (the bindings' warm per-thread
  arena reaches the same walk through the hidden `__private::parse_imports_in`). `Imports` holds
  `records: Vec<ImportRecord>`, in source order, and `has_module_syntax`. An `ImportRecord` is
  `Copy`: spans into the source (`ByteSpan`, UTF-8 byte offsets, `span.text(source)` the slice),
  never a copy of its text; `specifier_value(source)` decodes the specifier when asked,
  borrowing from the source when there is nothing to decode — `None` only for a record with no
  specifier, and a panic, as `span.text(source)` panics, when `source` is not the text the
  record came from. The error is the parse's
  `tsv_lang::Error`, as `parse_to_json` returns it. `tsv_ts::strip_with_imports(source)` (and
  `strip_with_imports_with_goal`) is the type strip and `parse_imports` from one parse and one
  walk — Rust only, no binding exposes it ([strip.md §Surfaces](./strip.md#surfaces)).
- **npm**: `parse_imports_typescript(source, options?)` and `parse_imports_svelte(source,
  options?)` in `@fuzdev/tsv-wasm` and the native `@fuzdev/tsv` — the `imports` cargo feature
  of `tsv_wasm` and `tsv_napi`, in each crate's default set, which the format-only and
  parse-only wasm packages build without. Each returns a result instead of throwing for what it
  finds in the source, as every facade export does: `{ok: true, hasModuleSyntax, imports}` or
  `{ok: false, error}`, the error `parse_<lang>` returns. The TypeScript bag takes `sourceType`
  as a strip's does; the Svelte bag takes none (a `<script>` is always a module). Argument misuse
  throws a `TypeError`, as on every export
  ([crates/tsv_wasm/CLAUDE.md §The npm Facade](../crates/tsv_wasm/CLAUDE.md#the-npm-facade-options--typed-returns)).
- **CLI**: none. `tsv_debug imports <file>` prints one file's records, as a binding returns
  them, for triage.

## The record

| field | Rust | JS |
|---|---|---|
| the request's form | `kind: ImportRecordKind` | `kind`: `'static'` / `'re_export'` / `'dynamic'` / `'import_equals'` |
| the whole request | `span` | `start` / `end` |
| the specifier | `specifier: Option<ByteSpan>`, `specifier_value(source)` | `specifier: {value, start, end} \| null` |
| types alone | `type_only: TypeOnly` | `typeOnly`: `null` / `'statement'` / `'specifiers'` |
| the phase | `phase: ImportPhase` | `phase`: `null` / `'source'` / `'defer'` |
| attributes | `attributes: Option<ByteSpan>` | `attributes: {start, end} \| null` |

- **`kind`**. `static` is `import … from 'x'` and `import 'x'`; `re_export` is `export … from 'x'`
  and `export * from 'x'` (a re-export is a request too); `dynamic` is `import(…)`,
  `import.source(…)` and `import.defer(…)`; `import_equals` is `import x = require('x')`,
  `export import` and `import type` included. The entity-name form `import x = A.B` is an alias,
  not a request, and no record.
- **The span** is the statement for a static import, a re-export or an import-equals, the
  `import(…)` expression for a dynamic import.
- **The specifier** spans the literal's contents, its quotes or backticks excluded. Its value is
  decoded: a string literal's escapes, or a template literal's cooked value (a CR or CRLF read
  as LF) — the value tsv's parse wire carries for the literal. A dynamic import's specifier is
  a string literal or a template with no substitutions; any other argument is computed, and the
  record has no specifier.
- **`typeOnly`**. `statement` is `import type`, `export type`, `export type *` and
  `import type x = require('y')` — the type strip removes the statement. `specifiers` is every
  specifier marked inline `type` (`import {type A, type B} from 'x'`), at least one and none a
  default or namespace import — the strip keeps `import {} from 'x'`, so the stripped JS still
  loads the module, while a dependency graph over erased types may drop it. A dynamic import is
  never type-only.
- **`phase`** is the source-phase and deferred-import proposals' `import source x from 'y'` /
  `import.source('y')` and `import defer * as x from 'y'` / `import.defer('y')`.
- **`attributes`** spans a static import's or a re-export's clause from its `with` keyword
  through its closing `}`, or a dynamic import's options argument. The clause is not parsed into
  keys and values. The legacy `assert {…}` clause is not import-attribute syntax for tsv,
  which rejects it
  ([conformance_svelte.md §TypeScript Corrections](./conformance_svelte.md#typescript-corrections)).

## `hasModuleSyntax`

Whether the source uses syntax only a Module parses: an ECMAScript `import` or `export`
declaration at its top level (`import type`, `export type`, `export {}`, `export default` and a
local `export const` included), an `import.meta` anywhere outside types, or a top-level `await`,
`for await` or `await using` — outside every function, class static block, field initializer,
namespace body and enum member initializer, the last two function-like for `await` in tsc
(TS1308). This is the module-syntax core of what Node's ESM syntax detection reads; Node also
weighs other signals, such as a top-level redeclaration of a CommonJS wrapper name (`require`,
`module`, `exports`, `__filename`, `__dirname`), which are not module syntax and not read here.

Not module syntax: a dynamic `import(…)`, an `import x = require('y')`, an
`export import x = …` (either reference), an `export =`, an `export as namespace`, and an
`export` inside a namespace (TypeScript's namespace export). tsc's
module indicator is a different set: it adds an import-equals, an `export =` and any
`export`-modified declaration, and omits a top-level `await`. A source read at Script goal
never has module syntax — that grammar rejects all of it. A component answers for both its
scripts together.

## What is a record

The records are what the source requests at run time, in source order:

- **Ambient code holds none.** A `declare`d declaration, the body of a `declare module 'x'` /
  `declare global` / `declare namespace`, and a bodiless signature (an overload, an abstract
  member) describe types, not code that runs.
- **A namespace body holds dynamic imports alone.** A static import, a re-export or an
  import-equals inside a namespace (nested ones included) is no record: tsc rejects each there
  (TS1147, TS1194). An `import(…)` there is a record, as anywhere a value runs.
- **Every value position is walked**, the four the type strip refuses without walking included:
  an enum's member initializers, a value namespace's body, an `export =` operand, and the
  operand of an `as` / `satisfies` whose blanking would regroup the expression around it. A
  source the strip refuses still has all its records.
- **Types are not walked.** A computed key inside an interface or a type literal
  (`interface I { [import('./i')]: T }`) is an `import(…)` expression in the tree, but it sits in
  a type and runs nothing. So is a **type-position import** (`let x: import('./a').T`,
  `typeof import('./a')`) — a known gap, since a dependency graph over types would want it; a
  consumer building a runtime graph drops type-only requests anyway.
- **`import` that requests nothing is no record**: `import.meta`, an `import` key or member
  name, the word in a string or a comment.
- **A component's records are its scripts'**: the two top-level `<script>` blocks, in document
  order whichever comes first. A `<script>` nested in markup is markup, and a template
  expression (`{#await import('./X.svelte')}`) is not a script — Svelte's compile reads neither
  as the component's imports.

## The parse goal

`parse_imports_typescript` and `tsv_ts::parse_imports` read the goal as `format` and the strip
do: a named goal (`sourceType`, `parse_imports_with_goal`) is exact, and with none named the
source is parsed as a **module, retried as a script** only if that fails — so a legacy sloppy
script yields its dynamic imports and import-equals, while a module-valid source is never
reinterpreted ([strip.md §The parse goal](./strip.md#the-parse-goal), which states the rule for
both, the depth refusal and the reported error included). The JS family parses as TypeScript,
JSX out of scope. A component's scripts are modules.

## Coordinates

In Rust every span is a UTF-8 byte range into the source the records came from. At the JS
boundary every offset is a UTF-16 code unit, so `source.slice(start, end)` is the text, in the
language's own coordinates, as the parse wire's: a TypeScript source counts a leading
byte-order mark, as acorn does; a component's offsets index the document without it, as Svelte's
`parse` removes it. The specifier's value is decoded in Rust, the one thing a JS consumer cannot
read off a slice.

## Oracles

- **The fixtures** — `tests/fixtures_imports/`, graded in every `cargo test` by
  `tests/imports_fixtures_tests.rs`. Each `expected.json` is generated, never hand-written: an
  ESTree walk in `tsv_debug` (`imports_fixtures.rs`, its rules stated once as `RULES`) over the
  canonical parser's tree — acorn-typescript's, or Svelte's `parse` — which reads that tree
  alone, never tsv's AST, so it grades the collector independently; the generator also runs it
  over tsv's own parse wire and requires the same records. Import phases have no canonical
  parser (acorn-typescript rejects them), so their fixtures take the walk over tsv's wire and say
  so in an `oracle.txt`. `deno task imports:fixtures:validate` checks the committed files
  against the live oracle ([tests/fixtures_imports/README.md](../tests/fixtures_imports/README.md)).
- **The self-check** — the same walk over every canonical tree the main fixture tree already
  pins (`tests/fixtures/**/expected.json`), compared with `parse_imports` on each input at its
  goal: sidecar-free, in every `cargo test`
  ([audits.md §Import fixture self-check](./audits.md#import-fixture-self-check-imports_self_check_tests)).
- **The real-code differential** — `deno task imports:corpus`, tsv's records against
  oxc-parser's module record over the `../corpora` snapshot, every structural difference between
  the two models a named normalization
  ([audits.md §Import corpus](./audits.md#import-corpus-differential-importscorpus)).
- **The engines, the fuzzer and the depth gate** — `deno task engines:audit` holds the WASM and
  native engines to the same records; the fuzzer collects the records of every accepted mutant
  and checks their contract (sorted, disjoint or nested, in bounds, every specifier decoded);
  the depth gate walks every shape it builds at the parsers' limits
  ([audits.md](./audits.md)).

## Anti-features

- **No export bindings.** Which names a module exports is not a request; nothing consumes it.
- **No es-module-lexer compatibility.** The records are tsv's own shape, with a type-only
  verdict and import-equals records — es-module-lexer has no type-only notion and throws on
  `import x = require('y')`.
- **No configuration.**
