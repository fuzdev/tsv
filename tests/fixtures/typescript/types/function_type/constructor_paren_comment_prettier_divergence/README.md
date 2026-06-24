# Constructor type `new` to `(` comment — prettier divergence

Prettier moves block comments between `new` and `(` in constructor types
without type parameters inside the parens as leading on the first parameter:
`new /* c */ (x: number) => A` becomes `new (/* c */ x: number) => A`. With
empty params the comment lands after the `)` instead: `new /* c */ () => A`
becomes `new () /* c */ => A`. The `abstract` form behaves the same.

We preserve the comment between `new` and `(`: `new /* c */ (x: number) => A`
— matching the typed form `new <T>/* c */(...)` covered by
`../paren_in_comment_prettier_divergence/`.

Both positions are dual-stable (idempotent in both formatters;
`variant_inside_parens.svelte`). Per comment placement policy, we preserve
user intent.

Same principle as
`interfaces/construct_signature_paren_in_comment_prettier_divergence` (the
construct-signature form of the same gap).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation) (`new` to `(`).
