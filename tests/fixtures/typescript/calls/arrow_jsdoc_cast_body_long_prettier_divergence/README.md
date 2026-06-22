# Arrow callback with JSDoc type cast body (long)

In `.svelte` files, prettier-plugin-svelte preserves JSDoc type cast parens
(`/** @type {T} */ (fn(x))`), while tsv strips them (the parser consumes the
parens and returns the inner expression directly). This is the same paren
divergence documented in `jsdoc_type_cast_prettier_divergence` — and, as noted
there, the strip is **semantic** (it drops the cast in checkJs code), not
cosmetic.

When the arrow body line exceeds print width, a secondary line-breaking
difference appears: without parens, prettier (in `.ts` mode) and tsv both
see a `CallExpression` body and use the expand-last-arg pattern
(`map((x) =>\n  body`). But prettier-plugin-svelte sees
`ParenthesizedExpression` and falls back to standard arg breaking
(`map(\n  (x) => body`).

In standalone `.ts`/`.js` files there is no divergence — both formatters
strip the parens and use expand-last.

## Divergences (`.svelte` only)

1. **Paren stripping**: `/** @type {T} */ (fn(x))` → `/** @type {T} */ fn(x)`
   (same as `jsdoc_type_cast_prettier_divergence`)
2. **Line breaking** (long case only): we use expand-last `map((x) =>\n  body)`,
   prettier-plugin-svelte uses standard `map(\n  (x) => body)`

## Files

- `prettier_variant_with_parens.svelte` — prettier-plugin-svelte's stable form
  with parens preserved. Our formatter normalizes to `input.svelte` (strips parens).

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §TypeScript (JSDoc cast expand-last).
