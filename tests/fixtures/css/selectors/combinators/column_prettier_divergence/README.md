# column_prettier_divergence

Prettier removes spaces around the column combinator (`||`), which breaks Svelte's CSS parser.

tsv: `col.selected || td` (spaces preserved, Svelte-compatible)
Prettier: `col.selected||td` (spaces removed, Svelte parse error)

## Reason

Parser compat. Svelte's CSS parser requires spaces around `||` — without them it
fails with "Expected a valid CSS identifier". tsv prioritizes Svelte
compatibility. CSS Selectors Level 4 marks the column combinator as "At Risk".
See
[conformance_prettier.md §CSS: Selectors](../../../../../../docs/conformance_prettier.md#css-selectors).
