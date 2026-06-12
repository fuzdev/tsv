# Type Selector Escapes

Tests escape sequence preservation in type selectors.

**Critical**: `\:root` (element named ":root") ≠ `:root` (pseudo-class). Escapes must be preserved exactly.

## Examples

- `d\69v` → Decodes to `div` in AST, formatter preserves `d\69v` in output
- `\30span` → Decodes to `0span` in AST, formatter preserves `\30span` in output

## Parser Behavior

Svelte only decodes Unicode escapes (`\69`), not character escapes (`\:`). The tsv parser currently decodes both. This fixture tests Unicode escapes only.
