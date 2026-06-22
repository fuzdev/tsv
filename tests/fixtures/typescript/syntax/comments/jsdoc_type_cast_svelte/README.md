# JSDoc type cast parenthesization (Svelte / JS context)

tsv **preserves** the parentheses of a JSDoc type cast — `/** @type {T} */ (expr)`
— because they are semantically required (the parens are part of the cast; without
them it is not an assertion). This is a plain `<script>` (JS) component, so
prettier-plugin-svelte routes to babel and **also preserves** the parens — tsv
**matches** here (no divergence).

The divergence is JS-vs-TS: in TS contexts (`.ts`, `<script lang="ts">`) prettier's
oxc-ts backend strips the parens, where tsv diverges — see the sibling
[`jsdoc_type_cast_ts_prettier_divergence`](../jsdoc_type_cast_ts_prettier_divergence/)
fixture and [conformance_prettier.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier.md#jsdoc--paren-semantics).

## Contexts tested

Assignment RHS, reassignment, return, call argument, new-expression args, default
parameter values, destructuring defaults — each wrapping a JSDoc cast in parens.

## Related fixtures

- [jsdoc_type_cast_extent](../jsdoc_type_cast_extent/) — `(a.b)` vs `(a).b` cast extent
- [jsdoc_type_cast_nested](../jsdoc_type_cast_nested/) — nested casts
- [jsdoc_type_cast_keyword](../jsdoc_type_cast_keyword/) — `await`/`yield` casts
- [jsdoc_type_cast_member](../jsdoc_type_cast_member/), [jsdoc_type_cast_nonadjacent](../jsdoc_type_cast_nonadjacent/) — comments that are **not** casts (predicate negatives)
