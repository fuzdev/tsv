# Parser divergence: `prettier-ignore` member comments across construct kinds

This fixture covers `// prettier-ignore` on members of a class (field + method),
an enum, an interface, and a type literal — verifying the comment preserves the
member's original spacing in every case (the formatter output matches prettier).

The parser divergence is in **one** of those cases: the `// prettier-ignore`
between a type literal's `{` and its first member. That comment falls in a region
acorn-typescript re-parses — its backtrack-and-reparse fires the `onComment`
callback twice, so the comment is duplicated in the root `comments` array. The
class, enum, and interface member comments are parsed once by both parsers and do
not diverge. Our parser keeps a single entry everywhere (`expected_ours.json` vs
`expected_svelte.json`); the set of distinct comments is identical — only
multiplicity differs — and `ast_diff` confirms semantic equivalence.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
