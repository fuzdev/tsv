# Signature paren in comment — prettier divergence

Comments between type parameters and `(` in call/construct signatures where the
comment contains `(` (e.g., `/* a(b) */`). Covers both type literals and interfaces.

Prettier moves the comment inside parens as a leading comment on the first
parameter. tsv preserves the comment between `>` and `(`, consistent with the
comment position philosophy (don't move comments to different syntactic positions).

Same principle as `computed_key_bracket_paren_in_comment_prettier_divergence`.

Both positions are dual-stable under our formatter (`variant_inside_parens.svelte`).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
