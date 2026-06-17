# Parser divergence: angle-bracket type-assertion comment duplication

A comment inside an angle-bracket type assertion's `<…>` (`<\/* c *\/ string>x`)
falls in a region acorn-typescript re-parses: it first reads `<` as a less-than
operator, then backtracks and reparses as a type assertion, so its `onComment`
callback fires twice and the comment is duplicated in the root `comments` array.
Our parser keeps a single entry (`expected_ours.json` vs `expected_svelte.json`).
The set of distinct comments is identical — only multiplicity differs — and
`ast_diff` confirms semantic equivalence.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
