# Parser divergence: comment duplication in a mapped type's `{ [K in … ] }` header

A comment inside a mapped type's `{ [K in … ] }` header (before the `in`) falls in a region acorn-typescript re-parses — its backtrack-and-reparse
fires the `onComment` callback twice, so the comment is duplicated in the root
`comments` array. Our parser keeps a single entry (`expected_ours.json` vs
`expected_svelte.json`); the set of distinct comments is identical — only
multiplicity differs — and `ast_diff` confirms semantic equivalence.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
