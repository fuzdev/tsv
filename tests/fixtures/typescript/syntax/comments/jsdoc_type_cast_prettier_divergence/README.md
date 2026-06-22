# JSDoc type cast parenthesization

tsv strips parentheses from JSDoc type cast expressions:
`/** @type {T} */ (expr)` → `/** @type {T} */ expr`

This divergence is **Svelte-only**. In standalone `.ts`/`.js` files, prettier's
babel/typescript parser also strips the parens — both formatters agree.
The [non-divergence fixture](../jsdoc_type_cast/input.ts) confirms this.

In `.svelte` files, prettier-plugin-svelte preserves parens because acorn's
parser produces `ParenthesizedExpression` AST nodes, and the plugin keeps
them when preceded by a `@type`/`@satisfies` comment.

**Caveat — these parens are NOT semantically meaningless (re-examination).**
In checkJs / JSDoc-typed code `/** @type {T} */ (expr)` is a type **cast** and
the parens are required for the cast to apply — `/** @type {T} */ expr` is not a
cast. Stripping them silently drops the assertion. tsv strips to match
prettier's standalone TS/JS behavior, but in `.svelte` the canonical formatter
(prettier-plugin-svelte) deliberately *preserves* them, so here tsv both
diverges from the Svelte oracle and changes the meaning of typed components.
Whether tsv should preserve cast parens in `.svelte` (matching the plugin) is an
open question; this fixture pins the current strip behavior pending that call.

## Contexts tested

- Assignment RHS: `const a = /** @type {A} */ expr`
- Reassignment: `a = /** @type {A} */ expr`
- Return: `return /** @type {A} */ expr`
- Call argument: `fn(/** @type {A} */ expr)`
- New expression: `new A(/** @type {any} */ a, {})`
- Default parameter value: `function fn(a = /** @type {A} */ b) {}`
- Destructuring default: `function fn({a = /** @type {A} */ b} = {}) {}`

## Related fixtures

- [jsdoc_type_cast_member](../jsdoc_type_cast_member/) — member access and unary operators
  with JSDoc comments (comments preserved, paren stripping matches standalone TS)
- [jsdoc_type_cast_keyword](../jsdoc_type_cast_keyword/) — `await`/`yield` with JSDoc comments
- [jsdoc_type_cast](../jsdoc_type_cast/) — standalone `.ts` (no divergence)
