# container_long_prettier_divergence

Prettier does not implement line-width wrapping for `@container` queries. tsv wraps at 101+ chars.

tsv: wraps at 101 chars (>100)
Prettier: never wraps from compact input; preserves wrapping if input is already wrapped

| Line width | tsv    | Prettier (from compact) |
| ---------- | ------ | ----------------------- |
| 100 chars  | inline | inline                  |
| 101+ chars | wraps  | inline                  |

Prettier has stable variants: the inline form, the wrapped form, and a spaces-preserved form (multi-space around `and` kept, `prettier_variant_spaces`) are all idempotent under Prettier. tsv normalizes all of them to the wrapped form.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) (`@container line wrap`, Print width). tsv enforces printWidth consistently across all CSS at-rules; Prettier never implemented wrapping for `@container` queries.

## Related

- [media_long](../media_long_prettier_divergence/) — same: Prettier never wraps a single `@media` query
- [import_media_query_long](../import_media_query_long_prettier_divergence/) · [supports_long](../supports_long_prettier_divergence/) — off-by-one / off-by-two boundary variants
