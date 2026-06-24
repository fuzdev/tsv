# Parser divergence: comment duplication in an empty type-literal body (call type argument)

The type argument here is an empty object type literal `{ … }` (`fn<{ /* … */ }>()`).
A comment inside that empty `{ }` body falls in a region acorn-typescript
re-parses — its backtrack-and-reparse fires the `onComment` callback twice, so
the comment is duplicated in the root `comments` array. (The same empty
type-literal body duplicates in any type position, e.g. `Array<{ /* … */ }>`;
the call type-argument context here is incidental.) Our parser keeps a single
entry (`expected_ours.json` vs `expected_svelte.json`); the set of distinct
comments is identical — only multiplicity differs — and `ast_diff` confirms
semantic equivalence.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
