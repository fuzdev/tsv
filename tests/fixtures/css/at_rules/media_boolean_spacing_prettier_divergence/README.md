# media_boolean_spacing_prettier_divergence

Prettier bug: fails to add required spaces after `and`, `or`, `not` keywords before `(` in `@media` queries.

tsv: `@media screen and (min-width: 768px)` (spec-compliant)
Prettier: `@media screen and(min-width:768px)` (preserves compact form)

CSS Media Queries Level 4 requires whitespace between boolean keywords and `(` — without it, `and(...)` parses as a function token.

## Reason

Prettier correctly normalizes this for `@supports` but not `@media`:

| At-rule      | `and(...)` input      | Prettier output       |
| ------------ | --------------------- | --------------------- |
| `@supports`  | `@supports and(...)`  | `@supports and (...)` |
| `@media`     | `@media and(...)`     | `@media and(...)`     |

tsv normalizes both consistently per the spec.

## Related

- [container_spacing](../container_spacing_prettier_divergence/) — same bug for `@container`
