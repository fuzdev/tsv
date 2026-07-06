# Legacy import-assertions `assert` clause — Svelte Divergence

The abandoned Stage-3 predecessor of import attributes spelled the clause
`import x from 'm' assert { type: 'json' }`. It never merged into ecma262 — the
final grammar is `WithClause : with { … }`
([ecma262 §16.2.2](https://tc39.es/ecma262/#prod-WithClause)) — and engines have
since removed it.

**tsv** rejects the `assert` clause (`Expected ';'`), parsing only the spec's
`with` form. acorn-typescript still accepts the legacy `assert`, so this is
deliberate spec-over-acorn strictness — the reverse direction of most parser
corrections, where tsv is broader. The same divergence applies to
`export * from 'm' assert { … }` and `export { a } from 'm' assert { … }` (the
same `WithClause` parse path); the accepted `with` forms are covered by the
[attributes](../attributes/) fixture.

Because the canonical parser accepts the input, this rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject); the
`tsv_rejects.txt` marker pins tsv's rejection while `expected_svelte.json` proves
acorn still accepts.

**Upstream**: @sveltejs/acorn-typescript still accepts the removed `assert`
clause.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Legacy import-assertions `assert` clause).
