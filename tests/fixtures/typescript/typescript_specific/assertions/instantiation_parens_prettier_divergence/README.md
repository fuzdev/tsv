# instantiation_parens_prettier_divergence

Prettier strips parentheses around ternary and binary expressions in `TSInstantiationExpression`, changing the expression's semantics.

tsv: `(x ? y : z)<T>` (preserves semantics — instantiate ternary result)
Prettier: `x ? y : z<T>` (changes semantics — `<T>` only applies to `z`)

Same issue with binary: `(a + b)<T>` vs `a + b<T>`.

## Reason

**Semantic preservation.** Without parens, operator precedence changes. `x ? y : z<T>` means instantiate `z` only, not the whole ternary. This is the same principle as `(x ? y : z) as T` vs `x ? y : z as T`. tsv preserves semantics. Both formatters agree on preserving parens for assignment: `(x = y)<T>`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript (Instantiation expression parens).
