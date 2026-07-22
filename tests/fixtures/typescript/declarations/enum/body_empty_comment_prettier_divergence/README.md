# body_empty_comment_prettier_divergence

An empty `enum` body whose only content is an interior comment. A line-comment
body breaks multiline in both formatters (`enum A`, no divergence). An inline
block-comment body stays on one line, and there tsv keeps the braces spaced
while prettier 3.9.5 tightens them (`enum B`).

tsv: `enum B { /* block comment */ }`
Prettier: `enum B {/* block comment */}`

## Reason

An enum body is a brace body, so the same rule applies: tsv keeps bracket
spacing around a single-line comment-only body (a comment is content), where
prettier 3.9.5 tightens it. A truly empty `enum E {}` has no content to space
and stays tight in both. Bracket spacing is hardcoded in tsv, so this is a fixed
design choice, not a configurable gap. Same rule as the object-literal
([empty_block_comment](../../../expressions/objects/empty_block_comment_prettier_divergence/)),
destructuring-pattern
([empty_comment](../../../expressions/destructuring/empty_comment_prettier_divergence/)),
and type-literal
([literal_body_empty](../../../types/comments/literal_body_empty_prettier_divergence/))
forms.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Empty-object comment bracket spacing.
