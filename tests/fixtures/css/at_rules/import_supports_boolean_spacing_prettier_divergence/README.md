# import_supports_boolean_spacing_prettier_divergence

Spec violation: Prettier preserves a boolean keyword (`and`, `or`, `not`) jammed against `(` inside an `@import` prelude's `supports()` argument. tsv inserts the spec-required space — the same condition text, in the same shape, that it (and prettier) already space in `@supports` position.

tsv: `supports((display: grid) and (gap: 1px))` (spec-compliant)
Prettier: `supports((display: grid) and(gap: 1px))` (preserves the glued form)

CSS Media Queries Level 4 §3 requires whitespace between a boolean keyword and `(` — without it, `and(...)` tokenizes as a `<function-token>` (CSS Syntax 3 §4.3.4). `supports( <supports-condition> | <declaration> )` (css-cascade-5 §"Conditional import rules") is the same `<supports-condition>` grammar `@supports` takes, so tsv parses the argument with the same reader and prints it with the same printer: one condition, one form, wherever it appears. Both forms are prettier-stable, so the divergence is reachable only from the glued authoring — pinned as a `prettier_variant_*`.

Prettier is inconsistent with itself here rather than with the spec alone: its `@supports` printer un-glues the very same text (`@supports (display: grid) and(gap: 1px)` → `and (gap: 1px)`), while its `@import` prelude is value-parsed, where `and(` reads as a function node whose name stays glued to its `(`.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) for the spec basis. Prettier normalizes this for `@supports` but not `@media`, `@container`, or `@import`'s `supports()`:

| Position               | `and(...)` input                | Prettier output                 |
| ---------------------- | ------------------------------- | ------------------------------- |
| `@supports`            | `@supports and(...)`            | `@supports and (...)`           |
| `@media`               | `@media and(...)`               | `@media and(...)`               |
| `@container`           | `@container and(...)`           | `@container and(...)`           |
| `@import` `supports()` | `@import url(a) supports(...and(...))` | `...and(...)` (glued) |

tsv normalizes all four consistently per the spec.

## Related

- [media_boolean_spacing](../media_boolean_spacing_prettier_divergence/) — same spec violation for `@media`
- [container_spacing](../container_spacing_prettier_divergence/) — same spec violation for `@container`
- [import_supports_condition](../import_supports_condition/) — the rest of the `supports()` argument, where tsv and prettier agree
