# container_spacing_prettier_divergence

Spec violation: Prettier does not normalize spacing in `@container` query preludes — both missing spaces and extra spaces are preserved verbatim. tsv normalizes to the spec-required single space.

tsv: normalizes to spec-compliant single spaces (`(min-width: 700px) and (max-width: 1000px)`)
Prettier: preserves whatever spacing the input has (`(min-width:700px)and(max-width:1000px)`)

**Missing spaces** — Prettier keeps compact forms like `(min-width:700px)` and `not(min-width:400px)` instead of adding required spaces.

**Extra spaces** — Prettier preserves `name1     (min-width: 400px)` instead of normalizing to single space.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) for the spec basis. CSS Media Queries Level 4 §3 requires whitespace between boolean operator keywords (`not`, `and`, `or`) and `(` — without it, `and(...)` tokenizes as a `<function-token>` (CSS Syntax 3 §4.3.4). Container Queries (CSS Conditional 5) use the same grammar. Prettier only normalizes this for `@supports`, not `@media` or `@container`:

| At-rule      | `and(...)` input      | Prettier output       |
| ------------ | --------------------- | --------------------- |
| `@supports`  | `@supports and(...)`  | `@supports and (...)` |
| `@media`     | `@media and(...)`     | `@media and(...)`     |
| `@container` | `@container and(...)` | `@container and(...)` |

## Related

- [media_boolean_spacing](../media_boolean_spacing_prettier_divergence/) — same spec violation for `@media`
