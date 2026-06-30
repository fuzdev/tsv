# JSDoc type cast parentheses (standalone TS)

tsv **preserves** the parentheses of a JSDoc type cast — `/** @type {T} */ (expr)`
— everywhere, because in checkJs / JSDoc-typed code the parens are part of a type
**cast**: `/** @type {T} */ (expr)` asserts the type, whereas `/** @type {T} */ expr`
does *not* (TypeScript requires the parentheses). Stripping them silently drops the
assertion, so preserving is the semantically faithful behavior.

This is a **`_prettier_divergence` in TS contexts** (`.ts`, `.js`, and
`<script lang="ts">`): prettier's TypeScript parser (oxc-ts) is cast-unaware and
**strips** the parens (see `output_prettier.ts`). In **JS contexts** (plain
`<script>`, babel) prettier-plugin-svelte *preserves* them and tsv **matches** —
see the sibling [`jsdoc_type_cast_svelte`](../jsdoc_type_cast_svelte/) fixture.

tsv runs one context-free TypeScript formatter for both JS and TS, so "preserve"
is uniform: it matches prettier in JS contexts and diverges in TS contexts.

See [conformance_prettier.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier.md#jsdoc--paren-semantics).

## Contexts tested

- Assignment RHS, reassignment, **assignment target** (the cast wraps the place
  being assigned, including compound `op=` — e.g. `/** @type {A} */ (g.h) += expr`),
  return, call argument, new-expression args, default parameter values,
  destructuring defaults — each wrapping a JSDoc cast.

## Related fixtures

- [cast_target_jsdoc](../../../expressions/assignment/cast_target_jsdoc/) — the same
  JSDoc-cast assignment target in a **JS** `<script>` (a **match** — prettier-JS
  preserves), beside the `(x as T)` sibling `cast_target` fixtures
- [jsdoc_type_cast_svelte](../jsdoc_type_cast_svelte/) — JS `<script>`, the same
  forms as a **match** (prettier-JS preserves)
- [jsdoc_type_cast_extent](../jsdoc_type_cast_extent/) — `(a.b)` vs `(a).b` cast extent
- [jsdoc_type_cast_nested](../jsdoc_type_cast_nested/) — nested casts
- [jsdoc_type_cast_member](../jsdoc_type_cast_member/), [jsdoc_type_cast_nonadjacent](../jsdoc_type_cast_nonadjacent/) — comments that are **not** casts (predicate negatives)
