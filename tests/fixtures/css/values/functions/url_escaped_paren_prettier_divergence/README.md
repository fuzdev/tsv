# url_escaped_paren_prettier_divergence

`url(a\)b)` — an unquoted `url()` whose content contains an **escaped** `)`
(`\)`). Per CSS Syntax 3 §4.3.6 a url-token consumes to the first *unescaped*
`)`, so `\)` is literal content and the final unescaped `)` closes the token; the
value is `url(a\)b)`. Svelte's `parseCss` accepts it (matching the spec) and tsv
keeps it stable.

Prettier's CSS parser (postcss, not `typescript`) miscounts the escaped `)` as a
closing paren and throws:

```
Unbalanced parenthesis
```

Same "prettier rejects valid input" shape as
[supports_unbalanced_paren](../../../at_rules/supports_unbalanced_paren_prettier_divergence/):
tsv (and parseCss) follow the spec where prettier's postcss is buggy.
`expected.json` is the parseCss AST; `prettier_rejects.txt` holds prettier's error
substring (no `output_prettier.*` — prettier can't format it).

See [conformance_prettier.md §Prettier rejects valid input](../../../../../../docs/conformance_prettier.md#prettier-rejects-valid-input).
