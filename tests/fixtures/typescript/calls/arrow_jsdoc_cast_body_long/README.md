# Arrow callback with JSDoc type cast body (long)

tsv **preserves** JSDoc type cast parens (`/** @type {T} */ (fn(x))`) — they are
required for the cast. This is a plain `<script>` (JS) component, so
prettier-plugin-svelte routes to babel and preserves them too: tsv **matches**.

When the arrow body line exceeds print width, the preserved parens make the body
**layout-opaque** — the cast hides the inner `CallExpression` from the
expand-last-arg heuristic (mirroring acorn's `ParenthesizedExpression`), so both
tsv and prettier-plugin-svelte fall back to **standard arg breaking**
(`map(⏎ (x) => /** @type */ (fn(x))⏎)`), not expand-last. No divergence.

In TS contexts (`.ts`, `<script lang="ts">`) prettier's oxc-ts backend strips the
parens (and would then use expand-last on the bare call) — that is the JS-vs-TS
divergence documented for the cast family. See
[conformance_prettier.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier.md#jsdoc--paren-semantics)
and the base divergence fixture `jsdoc_type_cast_ts_prettier_divergence`.
