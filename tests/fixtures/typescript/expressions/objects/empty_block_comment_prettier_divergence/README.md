# empty_block_comment_prettier_divergence

An empty object **literal** (value position) whose only body content is an
inline block comment — `const o = { /* c */ }`. tsv keeps the braces spaced;
prettier 3.9.5 tightens them to `{/* c */}`.

tsv: `const o = { /* c */ };`
Prettier: `const o = {/* c */};`

(A line-comment body breaks multiline in both — no divergence there; that case
is the non-divergent [empty_comment](../empty_comment/) fixture.)

## Reason

tsv applies bracket spacing uniformly: any object body kept on one line gets the
` … ` padding, a comment-only body included — a comment is content. It is not
special-cased on emptiness (a truly empty `{}` has no content to space and stays
tight in both). Prettier 3.9.5 strips the padding when the sole body content is a
comment. Bracket spacing is hardcoded in tsv, so this is a fixed design choice,
not a configurable gap. Same rule as the destructuring-pattern
([empty_comment](../../destructuring/empty_comment_prettier_divergence/)), enum
([body_empty_comment](../../../declarations/enum/body_empty_comment_prettier_divergence/)),
and type-literal
([literal_body_empty](../../../types/comments/literal_body_empty_prettier_divergence/))
forms — every comment-only empty brace body spaces.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Empty-object comment bracket spacing.
