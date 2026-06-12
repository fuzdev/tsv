# supports_long_prettier_divergence

Prettier wraps `@supports` queries two chars later than tsv (off-by-two).

tsv: wraps at 101 chars (>100)
Prettier: wraps at 103 chars (>102)

| Line width | tsv   | Prettier |
| ---------- | ----- | -------- |
| 100 chars  | inline | inline  |
| 101 chars  | wraps  | inline  |
| 102 chars  | wraps  | inline  |
| 103 chars  | wraps  | wraps   |

## Reason

Prettier has an off-by-two quirk for `@supports` queries. tsv wraps at exactly 101 chars for consistency with other CSS constructs.

## Related

- [import_media_query_long](../import_media_query_long_prettier_divergence/) — off-by-one variant
- [transform_long](../../values/functions/transform_long_prettier_divergence/) — similar off-by-one in values
