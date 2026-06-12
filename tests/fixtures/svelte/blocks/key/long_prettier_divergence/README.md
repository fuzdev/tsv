# long_prettier_divergence

Prettier keeps long expressions inline in `{#key}` blocks even when they exceed print_width.

tsv: wraps expressions at 101+ chars
Prettier: keeps inline (exceeds print_width)

| Line width | tsv   | Prettier |
| ---------- | ----- | -------- |
| 100 chars  | inline | inline  |
| 101+ chars | wraps  | inline  |

## Reason

tsv wraps block expressions consistently with how TypeScript formats the same expressions in `<script>` tags. Consistent with tsv's handling of `{#await}`, `{#each}`, and `{#if}` long expressions.
