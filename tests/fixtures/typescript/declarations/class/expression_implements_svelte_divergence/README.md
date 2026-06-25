# Parser divergence: anonymous class-expression `id` for implements-first heritage

acorn-typescript omits the `id` key entirely from an anonymous class
*expression* whose first heritage clause is `implements` with no name, type
parameters, or `extends` (`class implements Contract {}`) — yet it emits
`id: null` for every other anonymous class (`class {}`, `class extends Base {}`,
`class<T> implements Contract {}`). ESTree specifies `id: Identifier | null`
(always present), so our parser emits `id: null` consistently across all
anonymous classes (`expected_ours.json` vs `expected_svelte.json`); the `id`
key is the only difference, and `ast_diff` confirms semantic equivalence.

Formatting is unaffected — the input formats to itself under both tsv and
prettier (this is a parse-AST metadata divergence only, with no
`output_prettier.svelte`).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§TypeScript Corrections.
