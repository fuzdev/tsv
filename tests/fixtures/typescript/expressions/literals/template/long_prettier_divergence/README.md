# long_prettier_divergence

Tests the 100/101 char printWidth boundary for template literal interpolations.

tsv: breaks to `${\n\texpr\n}` at 101 chars
Prettier: keeps `${expr}` inline (exceeds printWidth)

| Line width | tsv                | Prettier           |
| ---------- | ------------------ | ------------------ |
| 100 chars  | `${expr}` inline   | `${expr}` inline   |
| 101 chars  | `${\n\texpr\n}`    | `${expr}` inline   |

## Reason

tsv treats printWidth as a hard limit for template interpolations. Consistent with tsv's template literal handling across all contexts.
