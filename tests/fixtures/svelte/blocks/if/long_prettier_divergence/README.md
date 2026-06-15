# long_prettier_divergence

Prettier keeps long binary expressions inline in `{#if}` block conditions even when they exceed printWidth.

tsv: wraps with continuation indent at 101+ chars
Prettier: keeps inline (exceeds printWidth)

| Line width | tsv   | Prettier |
| ---------- | ----- | -------- |
| 100 chars  | inline | inline  |
| 101+ chars | wraps  | inline  |

Tests binary expressions, function calls in binaries, and `{:else if}` conditions.

## Reason

tsv wraps block expressions consistently with how TypeScript formats the same expressions in `<script>` tags. Consistent with tsv's handling of `{#await}`, `{#each}`, and `{#key}` long expressions.
