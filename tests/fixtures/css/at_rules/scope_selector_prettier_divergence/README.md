# scope_selector_prettier_divergence

Prettier preserves newlines in `@scope` selector lists. tsv normalizes to inline format.

tsv: `@scope (.x, .y) {` (inline)
Prettier: preserves `@scope (.x,\n.y) {` (newline kept)

Prettier normalizes newlines in `:is()` and `:where()` selector lists but not `@scope`.

## Reason

tsv normalizes all selector lists consistently, regardless of at-rule context.

## Related

- [scope_complex](../scope_complex_prettier_divergence/) — whitespace inside parens (separate quirk)
