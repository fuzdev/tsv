# media_boolean_spacing_prettier_divergence

Spec violation: Prettier preserves a compact `@media` prelude where a boolean keyword (`and`, `or`, `not`) is jammed against `(`. tsv inserts the spec-required space.

tsv: `@media screen and (min-width: 768px)` (spec-compliant)
Prettier: `@media screen and(min-width:768px)` (preserves compact form)

CSS Media Queries Level 4 §3 requires whitespace between a boolean keyword and `(` — without it, `and(...)` tokenizes as a `<function-token>` (CSS Syntax 3 §4.3.4). Prettier's preserved form is valid stable output but non-normalized; tsv normalizes per the spec.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) for the spec basis. Prettier normalizes this for `@supports` but not `@media`:

| At-rule      | `and(...)` input      | Prettier output       |
| ------------ | --------------------- | --------------------- |
| `@supports`  | `@supports and(...)`  | `@supports and (...)` |
| `@media`     | `@media and(...)`     | `@media and(...)`     |

tsv normalizes both consistently per the spec.

## Related

- [container_spacing](../container_spacing_prettier_divergence/) — same bug for `@container`
