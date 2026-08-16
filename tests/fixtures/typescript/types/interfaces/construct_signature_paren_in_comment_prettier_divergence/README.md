# Construct signature `new` to `(` comment — prettier divergence

Prettier moves block comments between `new` and `(` in construct signatures
without type parameters inside the parens as leading on the first parameter:
`new /* c */ (a: number): A` becomes `new (/* c */ a: number): A`. With empty
params the comment lands after the `)` instead: `new /* c */ (): A` becomes
`new () /* c */ : A`. Line comments move into expanded multiline params.

We preserve the comment between `new` and `(`: `new /* c */ (a: number): A`,
a line comment staying on the `new` line with the params dropping to a
continuation line indented one level (`new // c` then `\t(a: number): A;` —
[§Uniform Forced-Continuation
Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent),
the same indent this gap's constructor-type twin below already took) — matching
the typed form `new /* c */ <T>(...)`, which both formatters keep in place
(see `../construct_signature_comment/`). Covers interfaces and type literals,
including comments containing `(`.

Both positions are dual-stable (idempotent in both formatters;
`variant_inside_parens.svelte`). Per comment placement policy, we preserve
user intent.

Same principle as `type_members/signature_paren_in_comment_prettier_divergence`
(the type-params-to-`(` family) and
`function_type/constructor_paren_comment_prettier_divergence` (constructor
types).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation (`new` to `(`).
