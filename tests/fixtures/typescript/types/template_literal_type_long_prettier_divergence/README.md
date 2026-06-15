# template_literal_type_long_prettier_divergence

When a template literal type exceeds printWidth, both formatters break at `=` for type aliases. After the `=` break, Prettier keeps the template literal inline even if it still exceeds printWidth. tsv also breaks at `${` if the line remains over 100 chars.

tsv: breaks at `=` AND `${` when needed
Prettier: breaks at `=` only, keeps template inline (exceeds printWidth)

Tests single interpolation, multiple interpolations (one long), and multiple interpolations (several long). Each `${...}` that exceeds 100 chars breaks independently.

See `template_literal_type_long/` for matching cases at the 100-char boundary.

## Reason

tsv applies the same printWidth rules after `=` breaks — consistent with template literal value handling.

## Related

- [template_literal_type_conditional_long](../template_literal_type_conditional_long_prettier_divergence/) — conditional types break at `?`/`:` instead of `${`
