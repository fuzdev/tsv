# Overload/abstract type params paren comment divergence

Prettier moves block comments between type parameters `>` and opening `(` inside
the parentheses as leading on the first parameter, for body-less function
overloads (`;` signature) and abstract class methods: `m<T>(/* c */ x): void`.

We preserve the comment between `>` and `(`: `m<T> /* c */(x): void`.

Both positions are dual-stable (idempotent in both formatters). Per comment
placement policy, we preserve user intent. Function-likes with a body (where
prettier also preserves the comment between `>` and `(`) are in
`type_params_paren_comment`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
