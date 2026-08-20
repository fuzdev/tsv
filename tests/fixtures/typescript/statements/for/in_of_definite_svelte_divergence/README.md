# Definite `!` on a `for`-in/of binding — Svelte Divergence

The `for`-in/of left and the C-style init are one production in TypeScript's
parser: both come from `parseVariableDeclarationList(/*inForStatementInitializer*/
true)`, which selects the `allowExclamation: false` declarator for the whole `for`
head. So a definite-assignment `!` is barred on the `of`/`in` binding for exactly
the reason it is barred on the init — position, not shape.

**tsv** follows tsc: the `!` fails (`a definite assignment assertion is not
permitted in a for header`). tsc rejects `for (const a!: number of xs)` with six
`parseDiagnostics` (the first `',' expected.`), and prettier — whose `typescript`
parser is tsc — rejects too.

The position is the *only* defect: `for (const a: number of xs)` parses with
**zero** diagnostics, parser and checker alike.

## Why tsv Differs

**acorn-typescript accepts**, building the `VariableDeclarator` it builds for a
variable statement — `definite: true` with the `number` annotation — having no
for-header parameter at all. tsv built that tree until this rejection landed, and
its printer then dropped the `!`, emitting `for (const a: number of xs)`: a silent
deletion of authored source that re-parsed as a different program.

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for
validity the oracle is tsc, which rejects **in its parser** rather than its
checker — the line that separates this from the definite-marker cases tsv defers
(TS1263 / TS1264, `checkGrammar*` diagnostics over a clean parse).

## The boundary

The C-style init half is [init_definite](../init_definite_svelte_divergence/),
which carries the full argument. The `for await` spelling reaches the same
declaration-list parse, as do `using` / `await using` — the latter pinned as an
ordinary `input_invalid_*` file in
[using/basic](../../../typescript_specific/using/basic_svelte_divergence/), acorn
having no `using` declarations to accept.

A marker-free binding is unaffected in every spelling —
[for_in_of](../for_in_of/) is the fixed point.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
