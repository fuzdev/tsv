# Arrow type params paren comment divergence

Prettier moves block comments between type parameters `>` and opening `(` inside
the parentheses as leading on the first parameter: `<T,>(/* c */ x: T) => x`.

We preserve the comment between `>` and `(`: `<T,> /* c */(x: T) => x`.

Both positions are dual-stable (idempotent in both formatters). Per comment
placement policy, we preserve user intent.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
