# block_value_svelte_prettier_divergence

CSS Variables Module Level 1 §2.1 defines a custom property's value as
`<declaration-value>` (CSS Syntax Level 3 §4.3.7) — any sequence of one or
more tokens with balanced `()` / `[]` / `{}` brackets. That includes a
top-level `{...}` block as the entire value.

This pattern appears in prettier's own corpus
(`prettier/tests/format/css/custom-properties/emoji.css`).

## Divergences

**Svelte (parser)**: rejects with `Expected a valid CSS identifier`. Svelte's
CSS parser does not accept the block-value form.

**Prettier (formatter)**: accepts and formats the block contents on their own
lines (indented like a nested rule body, with the closing `}` on its own line
followed by `;`).

**tsv**: accepts (parser matches spec, not Svelte) and preserves the value
as a single-line expression. The block contents are treated as opaque tokens
in the value position; we do not re-indent them like a nested rule body.

A future printer pass could match prettier's wrapped output. For now the
single-line form is stable and idempotent under tsv; the prettier-wrapped
form (captured in `output_prettier.svelte`) is what prettier produces from
that same input.

## Fixture Structure

- `input.svelte` — tsv canonical form (single-line block value)
- `output_prettier.svelte` — prettier's formatted form (multi-line)
- `expected_ours.json` — tsv AST with the custom property's value spanning the
  raw `{ ... }` block source
- `expected_svelte.json` — `{"error": "failed to parse"}` (Svelte parse failure)
