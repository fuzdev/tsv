# scope_complex_prettier_divergence

Stable quirk: Prettier preserves arbitrary whitespace in `@scope` preludes instead of normalizing — each spacing form is idempotent under Prettier.

tsv: `@scope (.class1) to (.class2)` (normalized single spaces)
Prettier: preserves `@scope ( .class1 )`, `(.class1,.class3)`, `( .class1 )  to  ( .class2 )` (compact / spaces / comma-space-before forms — see the `prettier_variant_*` files)

Prettier normalizes whitespace inside parens for `@media` and `@supports` but not `@scope`.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) (`@scope whitespace`, Stable quirk). tsv normalizes all at-rule preludes consistently. CSS Cascade Level 6 defines the `@scope` prelude as selector lists with standard whitespace rules.

## Related

- [scope_selector](../scope_selector_prettier_divergence/) — newline preservation (separate quirk)
