# `asserts` split from the asserted name (`asserts` ⏎ `a`) — Svelte Divergence

`asserts` is a contextual keyword: it heads a `TSTypePredicate` only when the
asserted name follows on the **same line**. TypeScript's parser gates the whole
reading on `lookAhead(nextTokenIsIdentifierOrKeywordOnSameLine)`, so `asserts`
across a newline is the ordinary type reference `asserts` and the name that
follows begins nothing valid.

**tsv** follows tsc: the return type ends at `asserts`, leaving `a {}` with no
parse (`Expected ';'`). tsc rejects with **TS1434 "Unexpected keyword or
identifier"**, and prettier — whose `typescript` parser is tsc — rejects too.

## Why tsv Differs

**acorn-typescript accepts**, welding across the break into the very
`TSTypePredicate` it builds for the same-line spelling — `parameterName: a`,
`asserts: true`, `typeAnnotation: null` — so the line terminator simply vanishes.
tsv rejects instead.

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for
validity the oracle is tsc, which rejects. A tree that silently discards a line
terminator tsc treats as fatal is worse than no tree — the same call made for
[declare/export_line_break](../../declare/export_line_break_svelte_divergence/)
and [decorators/declare_line_break](../../decorators/declare_line_break_svelte_divergence/).

Per ecma262 §sec-comments a `MultiLineComment` containing a line terminator *is*
a `LineTerminator`, so `asserts /*` ⏎ `*/ a` is the same rejection wearing trivia.

## The boundary

Only the `asserts`→name gap is restricted. These stay accepted, in tsv and tsc
alike:

- `(a: unknown): asserts a` — same line throughout
- `(a: unknown): asserts a is string` — the `is` clause is unaffected
- `(): asserts` — `asserts` alone is an ordinary type reference, so a name that
  never arrives is no error at all

The sibling rule one position over — `a` ⏎ `is T`, where the `is` keyword carries
the same same-line requirement — already rejects in tsv. The cases where `asserts`
is a parameter *name* rather than the modifier are pinned by
[predicate_param_named_asserts](../predicate_param_named_asserts/).

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
