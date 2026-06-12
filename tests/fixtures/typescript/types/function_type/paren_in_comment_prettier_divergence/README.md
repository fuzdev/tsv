# Function/constructor type paren in comment — prettier divergence

Comments between type parameters and `(` in function types (`<T>/* a(b) */(x: T) => void`)
and constructor types (`new <T>/* a(b) */(x: T) => A`) where the comment contains `(`.

Prettier moves the comment inside parens as a leading comment on the first
parameter. tsv preserves the comment between `>` and `(`, consistent with the
comment position philosophy.

Same principle as `type_members/signature_paren_in_comment_prettier_divergence`.

Both positions are dual-stable under our formatter (`variant_inside_parens.svelte`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
