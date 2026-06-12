# comment_empty_value_svelte_divergence

A CSS declaration whose value is entirely a comment (`color: /* comment */;`).

Svelte 5.55.x strips block comments from the `value` field before validating, then rejects empty declarations with `css_empty_declaration`. Prettier still accepts and formats this syntax unchanged, so tsv follows Prettier (the formatter source of truth) and continues to parse the file.

tsv: parses to `Declaration { property: "color", value: "" }` (matches Svelte's comment-stripping rule, but does not reject the empty result)
Svelte: parse error — `Declaration cannot be empty` (https://svelte.dev/e/css_empty_declaration)

## Fixture Structure

- `expected_ours.json` — tsv AST with empty value after comment stripping
- `expected_svelte.json` — `{"error": "failed to parse"}` (Svelte parse failure, expected)
