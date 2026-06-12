# long_prettier_divergence

Prettier keeps long expressions inline in `{#each}` blocks even when they exceed print_width.

tsv: wraps expressions at 101+ chars
Prettier: keeps inline (exceeds print_width)

| Line width | tsv   | Prettier |
| ---------- | ----- | -------- |
| 100 chars  | inline | inline  |
| 101+ chars | wraps  | inline  |

## Reason

tsv wraps block expressions consistently with how TypeScript formats the same expressions in `<script>` tags. Consistent with tsv's handling of `{#await}`, `{#if}`, and `{#key}` long expressions.
