# Declare function paren in comment — prettier divergence

Comment containing `(` between type parameters and `(` in declare function
signatures (`declare function f<T>/* a(b) */(x: T): void`).

Prettier moves the comment inside parens as a leading comment on the first
parameter. tsv preserves the comment between `>` and `(`, consistent with the
comment position philosophy.

Same principle as `type_members/signature_paren_in_comment_prettier_divergence`.

Both positions are dual-stable under our formatter (`variant_inside_parens.svelte`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
