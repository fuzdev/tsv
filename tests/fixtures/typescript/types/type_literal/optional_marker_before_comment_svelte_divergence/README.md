# Parser divergence: comment duplication in a type literal's `{ … }` body

The members here carry a comment before the optional `?` marker
(`a /* c1 */?`, `m /* c2 */?`), but the duplicated comment is the leading one
between the type literal's `{` and its first member — that position falls in a
region acorn-typescript re-parses, and its backtrack-and-reparse
fires the `onComment` callback twice, so the comment is duplicated in the root
`comments` array. The before-`?` comments parse once. Our parser keeps a single
entry for every position (`expected_ours.json` vs `expected_svelte.json`); the
set of distinct comments is identical — only multiplicity differs — and
`ast_diff` confirms semantic equivalence.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
