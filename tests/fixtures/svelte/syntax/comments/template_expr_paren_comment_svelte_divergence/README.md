# Parser divergence: template-expression comment before a parenthesized subexpression

Svelte parses template expressions with `parse_expression_at`, which sets
acorn's `preserveParens: true` (see `svelte/.../1-parse/acorn.js`). A leading
comment before a parenthesized subexpression (`… && /* c */ (cc || dd)`) is
therefore attached by `add_comments` to the synthetic `ParenthesizedExpression`
wrapper. Svelte then runs `remove_parens` on the result
(`context.visit(node.expression)`), which **discards the wrapper and its
`leadingComments`** — so the comment survives only in the root `comments` array,
never attached to the inner expression.

**tsv keeps no span for a bare pair inside an expression** (parens are not
preserved as nodes, matching Svelte's *final* shape), so the comment attaches to
the inner expression (`expected_ours.json` vs `expected_svelte.json`). The pairs
it does keep a span for — a JSDoc cast's, and the bare pairs around an island's
root — run as the same discarded nodes and match Svelte. The comment is never lost:
both parsers keep it in the root `comments` array, the distinct-comment set is
identical, and `ast_diff` confirms semantic (code) equivalence.

Note this is **template-only**: in a plain `<script>`, Svelte's `parse` does
*not* set `preserveParens`, so the same comment attaches to the inner expression
in both parsers (no divergence). The pair here is precedence-required, so the
input is format-stable.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md#comment-attachment-differences)
§Comment Attachment Differences.
