# scope_complex_prettier_divergence

Prettier preserves arbitrary whitespace in `@scope` preludes instead of normalizing.

tsv: `@scope (.card) to (.ignore)` (normalized single spaces)
Prettier: preserves `@scope ( .card )`, `(.card,  .panel)`, `(.card)  to  (.ignore)`

Prettier normalizes whitespace inside parens for `@media` and `@supports` but not `@scope`.

## Reason

tsv normalizes all at-rule preludes consistently. CSS Cascade Level 6 defines `@scope` prelude as selector lists with standard whitespace rules.

## Related

- [scope_selector](../scope_selector_prettier_divergence/) — newline preservation (separate quirk)
