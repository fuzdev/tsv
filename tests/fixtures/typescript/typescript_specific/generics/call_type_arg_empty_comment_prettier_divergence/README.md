# call_type_arg_empty_comment_prettier_divergence

A call whose sole type argument is an empty object type literal with an interior
comment (`fn<{ /* empty */ }>()`, `fn<{ // empty }>()`).

## Formatting divergence (prettier)

tsv keeps the empty type argument's bracket spacing — `fn<{ /* empty */ }>()` —
where prettier 3.9.5 tightens the comment-only body to `fn<{/* empty */}>()`
(`const a`). A line-comment body breaks multiline and hugs in both (`const b`,
`fn<{ \n // empty \n }>()`) — no divergence there. The comment stays where the
author wrote it in both formatters; only the bracket padding differs.

This is the same bracket-spacing rule as
[literal_body_empty](../../../types/comments/literal_body_empty_prettier_divergence/)
and
[union_empty_object_member](../../../types/union_empty_object_member_prettier_divergence/),
pinned in call type-argument position. Prettier ≤3.9.4 additionally broke the
whole `<…>` list onto its own indented lines for the line-comment body where tsv
hugged; prettier 3.9.5 converged on the hug, leaving only the bracket spacing.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Empty-object comment bracket spacing.
