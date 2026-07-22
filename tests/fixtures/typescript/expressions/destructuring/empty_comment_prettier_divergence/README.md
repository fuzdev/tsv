# empty_comment_prettier_divergence

An empty object **destructuring pattern** whose only body content is an inline
block comment — `const { /* c */ } = x`. tsv keeps the braces spaced; prettier
3.9.5 tightens them to `{/* c */}`.

tsv: `const { /* c */ } = x;`
Prettier: `const {/* c */} = x;`

## Reason

A destructuring pattern's braces are syntactically an object brace body, so the
same rule applies: tsv keeps bracket spacing around a single-line comment-only
body (a comment is content), where prettier 3.9.5 tightens it. A truly empty
`{}` pattern has no content to space and stays tight in both. Bracket spacing is
hardcoded in tsv, so this is a fixed design choice, not a configurable gap. Same
rule as the object-literal
([empty_block_comment](../../objects/empty_block_comment_prettier_divergence/)),
enum
([body_empty_comment](../../../declarations/enum/body_empty_comment_prettier_divergence/)),
and type-literal
([literal_body_empty](../../../types/comments/literal_body_empty_prettier_divergence/))
forms.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Empty-object comment bracket spacing.
