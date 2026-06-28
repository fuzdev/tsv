# operand_paren_comment_prettier_divergence

A block comment trailing a **parenthesized binary operand** of a unary operator
(`!((x + y) /* c */)`). The inner parens around `x + y` are redundant for
precedence, but they anchor the comment's association with the binary expression.

Prettier 3.9 strips the redundant parens and pulls the comment in against the
operand (`!(x + y /* c */)`); tsv keeps the parens so the comment stays
associated with the parenthesized sub-expression (`!((x + y) /* c */)`).

```ts
// prettier 3.9 (parens stripped)   // tsv (parens kept)
!(x + y /* c */);                    !((x + y) /* c */);
!(x || y /* c */);                   !((x || y) /* c */);
```

An assignment operand (`!((x = y) /* c */)`) keeps its parens in **both**
formatters (they're required), so it is a match. The non-binary operand cases
(`!(x /* c */)`, leading `!(/* c */ x)`, multiline, line-comment) also match.

## Reason

tsv preserves parentheses to keep a comment in place, the same approach as the
arrow-body stripped-parens case — preserving the comment's association is more
faithful than stripping the grouping parens. tsv never *adds* parens.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
