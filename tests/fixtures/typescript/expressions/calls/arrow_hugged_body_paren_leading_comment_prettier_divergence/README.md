# arrow_hugged_body_paren_leading_comment_prettier_divergence

A block comment leading an arrow's **object body**, written inside the parens the
grammar requires around it. Prettier moves it outside them
(`(x) => /* c */ ({ … })`); tsv keeps it where the author wrote it
(`(x) => (/* c */ { … })`), in every printer that hugs such a body — sole
argument, `new`, and the member chain's forced-expansion builder.

```
// tsv                            // prettier
fn((x) => (/* c */ {              fn((x) => /* c */ ({
	prop1: 'value1'                 prop1: 'value1'
}));                              }));
```

## Reason

Both positions exist in the printed output and both are stable, so the side of
the paren a comment sits on is authorship, not layout — moving it across the
delimiter is a relocation with no information to gain. The control (`b4`) writes
the comment outside and both formatters keep it there, which is what makes the
inside position a choice rather than a normalization.

The `_long` sibling
[arrow_hugged_body_paren_comment](../arrow_hugged_body_paren_comment_prettier_divergence/)
holds the trailing-comment reading of the same parens.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
