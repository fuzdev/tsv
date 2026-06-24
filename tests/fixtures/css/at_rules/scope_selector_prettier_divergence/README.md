# scope_selector_prettier_divergence

Stable quirk: Prettier preserves newlines in `@scope` selector lists. tsv normalizes to inline format.

tsv: `@scope (.x, .y) {` (inline)
Prettier: preserves `@scope (.x,\n.y) {` (newline kept)

Prettier normalizes newlines in `:is()` and `:where()` selector lists but not `@scope`.

## Reason

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules) (`@scope newlines`, Stable quirk). tsv normalizes all selector lists consistently, regardless of at-rule context.

## Related

- [scope_complex](../scope_complex_prettier_divergence/) — whitespace inside parens (separate quirk)
