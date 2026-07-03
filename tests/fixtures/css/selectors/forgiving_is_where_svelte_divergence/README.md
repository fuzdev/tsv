# forgiving_is_where_svelte_divergence

CSS Selectors Level 4 (Section 9.1) requires `:is()` and `:where()` to use forgiving parsing — invalid selectors are silently skipped, not errors.

tsv: forgiving parsing (spec-compliant) — `div:is(.a, ., .b)` → AST contains `.a` and `.b`
Svelte: strict parsing — any syntax error fails the entire parse

The forgiving list drops both **syntactically** invalid items (`.`, `[`) and items that are
**contextually** invalid — "known syntax but in an invalid context" (Selectors 4). An `An+B`
term (and its `of S` form) is valid only inside `:nth-*()`, so `:is(2n of)` is a contextually
invalid selector: the forgiving list drops it and `:is()` becomes empty, while
`:where(.class8, 2n of, .class9)` keeps the valid classes and drops only the `An+B`. tsv's
formatter preserves the dropped text verbatim (`2n of` stays in the output), matching prettier —
only the parsed AST omits it.

Pseudo-elements inside `:is()`/`:where()` are kept in the AST (contextually invalid for matching, but syntactically valid) — matching Svelte's behavior when given valid selectors.

## Fixture Structure

- `expected_ours.json` — tsv AST with syntax errors filtered, pseudo-elements kept
- `expected_svelte.json` — `{"error": "failed to parse"}` (Svelte parse failure, expected)

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).
