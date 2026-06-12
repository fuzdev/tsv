# Method type params paren comment divergence

Prettier moves block comments between type parameters `>` and opening `(` inside
the parentheses for interface method signatures and type literal method signatures.

We preserve the comment between `>` and `(`. Both positions are dual-stable
(idempotent in both formatters). Per comment placement policy, we preserve user intent.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
