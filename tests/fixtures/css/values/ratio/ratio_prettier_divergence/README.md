# ratio_prettier_divergence

Prettier preserves extra whitespace around `/` in ratio values inside media queries, but normalizes in property values.

tsv: `@media (aspect-ratio: 16 / 9)` (normalized everywhere)
Prettier: `@media (aspect-ratio: 16  /  9)` (preserves extra spaces in media queries)

Both formatters normalize `aspect-ratio: 16  /  9` to `aspect-ratio: 16 / 9` in property values.

## Reason

tsv normalizes whitespace consistently regardless of context (property value vs media query).
