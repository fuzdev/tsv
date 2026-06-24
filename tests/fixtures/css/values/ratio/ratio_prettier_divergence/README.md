# ratio_prettier_divergence

Prettier preserves extra whitespace around `/` in ratio values inside media queries, but normalizes in property values.

tsv: `@media (aspect-ratio: 16 / 9)` (normalized everywhere)
Prettier: `@media (aspect-ratio: 16  /  9)` (preserves extra spaces in media queries)

Both formatters normalize `aspect-ratio: 16  /  9` to `aspect-ratio: 16 / 9` in property values.

## Reason

Stable quirk. tsv normalizes whitespace consistently regardless of context (property value vs media query); prettier preserves the extra spaces inside a media-query ratio. See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values) ("Ratio in media queries").
