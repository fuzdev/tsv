# long_prettier_divergence

Prettier keeps long expressions inline in `{#await}` blocks even when they exceed printWidth.

tsv: wraps expressions at 101+ chars
Prettier: keeps inline (exceeds printWidth)

| Line width | tsv   | Prettier |
| ---------- | ----- | -------- |
| 100 chars  | inline | inline  |
| 101+ chars | wraps  | inline  |

## Reason

tsv wraps block expressions consistently with how TypeScript formats the same expressions in `<script>` tags. Consistent with tsv's handling of `{#each}`, `{#if}`, and `{#key}` long expressions.
