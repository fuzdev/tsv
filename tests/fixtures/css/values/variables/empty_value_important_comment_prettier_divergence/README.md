# empty_value_important_comment_prettier_divergence

An empty custom-property value carrying `!important` and a trailing comment
(`--a: !important /* c */;`). The comment is part of the declaration and tsv keeps it,
normalizing the empty value to a single space like
[empty_value_important](../empty_value_important_prettier_divergence/).

Prettier is **non-convergent** here as on the comment-free form — it adds a space before
`!important` on every pass — so `prettier_nonconvergent.txt` records the claim and the
validator live-verifies it (rule F5). The non-custom spelling, where prettier is stable and
agrees, is [css/tokens/important_empty_value](../../../tokens/important_empty_value/).

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values) ("Empty value + `!important`").
