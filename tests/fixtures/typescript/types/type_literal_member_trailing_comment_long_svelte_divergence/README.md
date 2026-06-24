# Parser divergence: comment duplication in a type literal's `{ … }` body

A `_long` fixture that also pins the union-member width boundary: member `a?` is
a union at exactly 100 columns (stays inline), `b?` is 101 columns (breaks into
leading-pipe multiline), and `c?` is a 100-column union followed by a trailing
`//` comment that pushes the line past 100 — the comment is excluded from the
`fits` measurement, so the union stays inline.

The parser divergence is the leading line comment between the type literal's `{`
and its first member — that position falls in a region acorn-typescript
re-parses, and its backtrack-and-reparse fires the `onComment` callback twice, so
the comment is duplicated in the root `comments` array. Our parser keeps a single
entry (`expected_ours.json` vs `expected_svelte.json`); the set of distinct
comments is identical — only multiplicity differs — and `ast_diff` confirms
semantic equivalence.

Formatting is unaffected by the duplication: the formatter finds comments by
position, not by their count in the root array, and emits each comment once at
the author's placement.
See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
