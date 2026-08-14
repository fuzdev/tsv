# Class property definite `!` after a line break (`a` ⏎ `!: T`) — Svelte Divergence

A definite-assignment `!` binds to the property name it follows only when no line
terminator intervenes. TypeScript's `parsePropertyDeclaration` takes the token
under `!scanner.hasPrecedingLineBreak()`, so `a` ⏎ `!: number` is the bodiless
field `a` (ASI) followed by a `!` that can head no class member.

**tsv** follows tsc: the member ends at `a`, and the stray `!` fails
(`Expected class member name`). tsc rejects with **TS1068 "Unexpected token. A
constructor, method, accessor, or property was expected"**, and prettier — whose
`typescript` parser is tsc — rejects too.

This is the guard tsv already applies to the **variable** spelling of the same
marker (`let x` ⏎ `!: number`), pinned by
[declarations/variable/definite_newline_invalid](../../../declarations/variable/definite_newline_invalid/)
— an ordinary `input_invalid_*` fixture, because acorn-typescript rejects that
one. On a class property it does not.

## Why tsv Differs

**acorn-typescript accepts**, welding across the break into the very
`PropertyDefinition` it builds for the same-line spelling — one member with
`definite: true` and the `number` annotation — so the line terminator simply
vanishes. tsv built that tree until this rejection landed.

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for
validity the oracle is tsc, which rejects. A tree that silently discards a line
terminator tsc treats as fatal is worse than no tree — the same call made for
[declare/export_line_break](../../../typescript_specific/declare/export_line_break_svelte_divergence/).

Per ecma262 §sec-comments a `MultiLineComment` containing a line terminator *is*
a `LineTerminator`, so `a /*` ⏎ `*/ !: number` is the same rejection wearing
trivia.

## The boundary

Only `!` is restricted; the **optional** `?` in the same syntactic slot is not.
`a` ⏎ `?: number` stays valid in tsc, acorn and tsv alike — tsc reads the `?`
without a line-break guard — so the two markers deliberately part ways here.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
