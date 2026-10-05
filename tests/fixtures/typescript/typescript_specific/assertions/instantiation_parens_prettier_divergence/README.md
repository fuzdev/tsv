# instantiation_parens_prettier_divergence

Prettier strips parentheses around ternary, binary and unary expressions in `TSInstantiationExpression`, changing the expression's semantics.

tsv: `(x ? y : z)<T>` (preserves semantics — instantiate ternary result)
Prettier: `x ? y : z<T>` (changes semantics — `<T>` only applies to `z`)

Same issue with binary: `(a + b)<T>` vs `a + b<T>`, and with unary: `(typeof e)<T>` vs `typeof e<T>`, where the operator takes the instantiation.

## Reason

**Semantic preservation.** Without parens, operator precedence changes. `x ? y : z<T>` means instantiate `z` only, not the whole ternary, and `typeof e<T>` is `typeof (e<T>)`. This is the same principle as `(x ? y : z) as T` vs `x ? y : z as T`. tsv preserves semantics. Both formatters agree on preserving parens for assignment: `(x = y)<T>`.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
