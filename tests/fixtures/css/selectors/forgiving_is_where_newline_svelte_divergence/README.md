# forgiving_is_where_newline_svelte_divergence

CSS Selectors Level 4 §4 requires `:is()` / `:where()` to use forgiving parsing
(`<forgiving-selector-list>` is syntactically `<any-value>?`): syntactically
invalid items are dropped from the AST, not errors. tsv is spec-compliant here;
Svelte's `parseCss` parses strictly and fails the whole parse — the sanctioned
divergence recorded in [`forgiving_is_where_svelte_divergence`](../forgiving_is_where_svelte_divergence/README.md).

This fixture pins the **formatter** side when the dropped item spans a newline.
The empty `.` class makes `.a > . > .b` an invalid complex selector, so it is
dropped from the AST; tsv keeps the dropped text in the output but **collapses
its internal whitespace runs — including embedded newlines — to single spaces**.
Prettier collapses the same whitespace, so ours matches prettier.

tsv & prettier: `div:is(.a > .⏎> .b)` → `div:is(.a > . > .b)` (newline collapsed)

## Reason

Prettier collapses whitespace runs (including newlines) inside a selector, so
tsv matching that keeps the dropped item's preserved text on one line — the same
rule tsv applies to every other selector-argument position. This is a formatter
choice (match prettier); the parser side — accepting the forgiving `:is()` where
parseCss rejects — is the recorded `_svelte_divergence`.

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).

## Fixture Structure

- `input.svelte` — the collapsed, single-line form (tsv-format-stable; parseCss rejects)
- `expected_ours.json` — tsv AST (the invalid item dropped from the `:is()` list)
- `expected_svelte.json` — `{"error": "failed to parse"}` (Svelte parse failure, expected)
- `unformatted_newlines.svelte` — the multi-line form (both tsv and prettier collapse to `input`)
