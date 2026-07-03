# supports_general_enclosed_svelte_divergence

`@supports` conditions that are valid `<general-enclosed>` but not a
`<supports-feature>` — `@supports (margin: 0;)` and `@supports foo(a; b)`. Per
CSS Conditional 3, a `<supports-in-parens>` may be a `<general-enclosed>` =
`( <any-value> )` or `<function-token> <any-value>? )`, and `<any-value>` admits
any *balanced* token run — including a `;`. So the condition parses (it evaluates
to *unknown* → false, but that is a cascade concern, not a parse error). tsv
follows the spec and prettier agrees (both keep the condition stable).

Svelte's `parseCss` instead rejects these — it truncates at the inner `;`,
reporting:

```
css_empty_declaration
```

This is the canonical-fails-tsv-ok pattern: tsv follows the CSS spec where
Svelte's parser is stricter than the grammar. It is the `@supports` sibling of
the declaration-value case
[function_semicolon](../../values/function_semicolon_svelte_divergence/).
`expected_ours.json` pins tsv's AST; `expected_svelte.json` records the Svelte
rejection.

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).
