# property_definite_comment_prettier_divergence

Prettier relocates comments between `!` (definite assignment) and `=` to before `!`:

- Input: `d! /* c */ = 1;`
- Prettier: `d /* c */! = 1;`
- Ours: `d! /* c */ = 1;` (preserves user placement)

Per comment placement policy, we preserve the user's original comment position.
Both forms are dual-stable (`variant_before_bang`, identical to prettier's
`output_prettier`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
