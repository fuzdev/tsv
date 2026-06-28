# call_type_arg_empty_comment_svelte_prettier_divergence

A call whose sole type argument is an empty object type literal with an interior
comment (`fn<{ /* empty */ }>()`, `fn<{ // empty }>()`). Two independent
divergences meet here.

## Formatting divergence (prettier)

tsv keeps the empty type argument hugged to the opening angle bracket. An inline
block comment stays inline in both formatters (`const a` — `fn<{ /* empty */ }>()`).
A line comment forces the body across lines: tsv keeps it hugged
(`fn<{ \n // empty \n }>()`), while prettier breaks the whole `<…>` list onto its
own indented lines (`const b`). The comment stays where the author wrote it in
both formatters — only the angle-bracket layout differs.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Single curly type-argument hug.

## Parser divergence (svelte/acorn)

A comment inside the empty `{ }` body falls in a region acorn-typescript
re-parses — its backtrack-and-reparse fires the `onComment` callback twice, so the
comment is duplicated in the root `comments` array. tsv keeps a single entry
(`expected_ours.json` vs `expected_svelte.json`); the set of distinct comments is
identical — only multiplicity differs — and `ast_diff` confirms semantic
equivalence. (The same empty type-literal body duplicates in any type position,
e.g. `Array<{ /* … */ }>`; the call type-argument context is incidental.)

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
