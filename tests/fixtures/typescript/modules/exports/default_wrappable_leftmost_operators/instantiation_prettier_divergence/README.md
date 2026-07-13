# instantiation_prettier_divergence

Prettier strips the parentheses around a `TSInstantiationExpression` whose operand is a class
expression in `export default` position — the same `◆prettier_bug` as the ternary/binary
instantiation cases, but here the strip is worse: it changes the AST.

tsv: `export default (class {}<T>);` — one statement, a `TSInstantiationExpression` (the class
expression instantiated with `<T>`)
Prettier: `export default class {}<T>;` — the leftmost `class {}` becomes a class _declaration_,
splitting into a `ClassDeclaration` plus a dangling `<T>;` `ExpressionStatement` (a different,
broken parse)

## Reason

**Semantic preservation.** In `export default` position the leading `class` keyword reads as a
(hoisted) class _declaration_ unless a `(` precedes it, so the parens are required to keep
`class {}` an _expression_. Prettier strips them (its `needs-parentheses` rule misses this case),
producing output that re-parses to a different AST. tsv keeps the parens — the same principle as
the ternary/binary instantiation strip, adjudicated by `export_default_needs_parens`.

See [conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript (Instantiation expression parens).
