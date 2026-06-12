# container_spacing_prettier_divergence

Prettier bug: fails to normalize spacing in `@container` query preludes — both missing spaces and extra spaces are preserved verbatim.

tsv: normalizes to spec-compliant single spaces (`(min-width: 700px) and (max-width: 1000px)`)
Prettier: preserves whatever spacing the input has (`(min-width:700px)and(max-width:1000px)`)

**Missing spaces** — Prettier keeps compact forms like `(min-width:700px)` and `not(min-width:400px)` instead of adding required spaces.

**Extra spaces** — Prettier preserves `name1     (min-width: 400px)` instead of normalizing to single space.

## Reason

CSS Media Queries Level 4 requires whitespace between boolean operator keywords (`not`, `and`, `or`) and `(` — without it, `and(...)` parses as a function token. Prettier only normalizes this for `@supports`, not `@media` or `@container`:

| At-rule      | `and(...)` input      | Prettier output       |
| ------------ | --------------------- | --------------------- |
| `@supports`  | `@supports and(...)`  | `@supports and (...)` |
| `@media`     | `@media and(...)`     | `@media and(...)`     |
| `@container` | `@container and(...)` | `@container and(...)` |
