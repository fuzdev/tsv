# empty_param_line_comment_svelte_divergence

A line comment inside an **empty** function-type or constructor-type parameter
list (`type F = (\n // c\n) => void`). The comment is the only content of the
parens, so it drops to its own indented line and breaks the `()` — there is no
parameter to trail.

## Format

tsv and Prettier agree: the comment lands on its own indented line and `=> void`
follows the `)` (`type F = (\n // c\n) => void;`). An inline block comment
(`(/* c */)`) stays inline. Covers function types and constructor (`new (...)`)
types.

This is the empty-params sibling of
[open_paren_comment](../open_paren_comment_prettier_divergence/) (non-empty
params, where tsv keeps the comment trailing `(` as an open-delimiter divergence
while Prettier drops it to the first parameter's leading line).

## Parser — svelte divergence

The same in-construct comment is duplicated in acorn-typescript's root
`comments` array (its backtrack-and-reparse fires `onComment` twice); our parser
keeps a single entry (`expected_ours.json` vs `expected_svelte.json`). The set of
distinct comments is identical — only multiplicity differs — and `ast_diff`
confirms semantic equivalence. See
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
