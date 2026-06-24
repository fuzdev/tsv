# Parser divergence: comment duplication in an index signature's `[k: T]` brackets

A comment **after the key** inside an index signature's `[k: T]` brackets
(`[key /* c */ : T]`) falls in a region acorn-typescript re-parses — its
backtrack-and-reparse fires the `onComment` callback twice, so the comment is
duplicated in the root `comments` array. Our parser keeps a single entry
(`expected_ours.json` vs `expected_svelte.json`); the set of distinct comments
is identical — only multiplicity differs — and `ast_diff` confirms semantic
equivalence. The other two cases place a `/* ] */` comment (whose body contains
a `]`) after the value type before the closing bracket, exercising
delimiter-scan robustness; those positions are not in the reparse window and
match canonical exactly (no duplication).

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
