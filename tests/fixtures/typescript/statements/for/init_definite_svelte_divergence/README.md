# Definite `!` on a `for` header's init declarator — Svelte Divergence

A definite-assignment `!` is part of the declarator production only where that
declarator heads a **variable statement**. TypeScript's `parseVariableDeclaration`
reads the token under three conjuncts — `allowExclamation && name.kind ===
Identifier && !scanner.hasPrecedingLineBreak()` — and
`parseVariableDeclarationList(/*inForStatementInitializer*/ true)` selects the
`allowExclamation: false` spelling for the whole `for` head. So the marker is
barred by position, exactly the way a grammar parameter (`[Await]`, `[Yield]`)
bars a production.

**tsv** follows tsc: the `!` fails (`a definite assignment assertion is not
permitted in a for header`). tsc rejects with **three** `parseDiagnostics`
(`',' expected.`, `Expression expected.`, `Declaration or statement expected.`),
and prettier — whose `typescript` parser is tsc — rejects too.

The position is the *only* defect here: `for (let a: number; ;)` and the
statement spelling `let a!: number;` each parse with **zero** diagnostics, parser
and checker alike. Nothing about the binding, the annotation or the empty clauses
is wrong — only that a `for` head is not a variable statement.

## Why tsv Differs

**acorn-typescript accepts**, building the `VariableDeclarator` it builds for the
statement spelling — `definite: true` with the `number` annotation — because it
has no for-header parameter at all. tsv built that tree until this rejection
landed, and its printer then dropped the `!` on the way out, emitting `for (let
a: number; ;)`: a silent deletion of authored source that re-parsed as a
different program.

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for
validity the oracle is tsc, which rejects **in its parser** rather than its
checker. That is what separates this from the definite-marker cases tsv *does*
defer — a bare `b!;` class property (TS1264) and `let a!: number = x;` (TS1263)
are `checkGrammar*` diagnostics over a clean parse, so tsv accepts and formats
them. Here `parseDiagnostics` is non-empty, so the rule is the parser's.

tsv already implemented two of tsc's three conjuncts: the `[no LineTerminator
here]` guard is pinned by
[declarations/variable/definite_newline_invalid](../../../declarations/variable/definite_newline_invalid/),
and the pattern arm never carried a marker. Accepting here was the remaining
conjunct going unasked — a hole in one position rather than a stance.

## The boundary

The `for`-in/of left takes the same rejection through the same declaration-list
parse — [in_of_definite](../in_of_definite_svelte_divergence/). The `using` and
`await using` spellings do too, but acorn has no `using` declarations at all, so
that one rides as an ordinary `input_invalid_*` file in
[using/basic](../../../typescript_specific/using/basic_svelte_divergence/).

Only the marker is barred, not the binding: `for (let a: number; ;)` stays valid
in tsc, acorn and tsv alike, and the statement-level `let a!: number;` keeps its
marker — [declarations/variable/definite_assignment](../../../declarations/variable/definite_assignment/).

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
