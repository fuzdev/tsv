# forgiving_is_where_svelte_divergence

CSS Selectors Level 4 (Section 9.1) requires `:is()` and `:where()` to use forgiving parsing — invalid selectors are silently skipped, not errors.

tsv: forgiving parsing (spec-compliant) — `div:is(.a, ., .b)` → AST contains `.a` and `.b`
Svelte: strict parsing — any syntax error fails the entire parse

Pseudo-elements inside `:is()`/`:where()` are kept in the AST (contextually invalid for matching, but syntactically valid) — matching Svelte's behavior when given valid selectors.

## Fixture Structure

- `expected_ours.json` — tsv AST with syntax errors filtered, pseudo-elements kept
- `expected_svelte.json` — `{"error": "failed to parse"}` (Svelte parse failure, expected)
