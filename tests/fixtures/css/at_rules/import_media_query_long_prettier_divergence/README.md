# import_media_query_long_prettier_divergence

Prettier wraps `@import` media conditions one char later than tsv (off-by-one).

tsv: wraps at 101 chars (>100)
Prettier: wraps at 102 chars (>101)

| Line width | tsv   | Prettier |
| ---------- | ----- | -------- |
| 100 chars  | inline | inline  |
| 101 chars  | wraps  | inline  |
| 102 chars  | wraps  | wraps   |

## Reason

tsv uses a consistent >100 threshold across all CSS constructs. Prettier has an off-by-one quirk for `@import` media conditions, same pattern as `@media`, `@supports`, and `@container`.

## Related

- [media_long](../media_long_prettier_divergence/) — same pattern, but Prettier never wraps
- [supports_long](../supports_long_prettier_divergence/) — off-by-two variant
- [container_long](../container_long_prettier_divergence/) — Prettier never wraps
