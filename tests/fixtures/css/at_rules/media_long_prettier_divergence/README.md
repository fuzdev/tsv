# media_long_prettier_divergence

Prettier does not wrap a **single** `and`-joined `@media` query (no top-level comma). tsv wraps it at the last fitting `and`/`or` boundary at 101+ chars.

tsv: wraps at 101 chars (>100)
Prettier: always keeps a single query inline, regardless of input formatting

| Line width | tsv    | Prettier |
| ---------- | ------ | -------- |
| 100 chars  | inline | inline   |
| 101+ chars | wraps  | inline   |

Unlike `@container` (where Prettier preserves wrapping if input is wrapped), Prettier always reformats a single `@media` query to inline — only one stable output form.

This applies only to a single query. A **comma-separated** query list is broken at the commas by both formatters (one query per line) — see the regular fixture `media_comma_long`; that is not a divergence.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) (`@media line wrap`, Print width). tsv enforces printWidth consistently across all CSS at-rules; Prettier never implemented intra-query (`and`/`or`) wrapping for `@media`.

## Related

- [container_long](../container_long_prettier_divergence/) — same: Prettier never wraps a single `@container` query
- [import_media_query_long](../import_media_query_long_prettier_divergence/) · [supports_long](../supports_long_prettier_divergence/) — off-by-one / off-by-two boundary variants
