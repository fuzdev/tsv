# template_literal_type_multibyte_long_prettier_divergence

Multibyte (CJK) companion to [template_literal_type_long](../template_literal_type_long_prettier_divergence/). The break-at-`${` decision for template literal types is a **visual-width** measure (CJK = 2 columns), not a byte count (CJK = 3 bytes), so a multibyte template type that fits within printWidth stays inline.

When a template literal type exceeds printWidth, both formatters break at `=` for type aliases. After the `=` break, Prettier keeps the template inline even if it still exceeds printWidth. tsv also breaks at `${` once the line is over 100 visual columns.

tsv: breaks at `=` AND `${` when the line exceeds 100 visual columns
Prettier: breaks at `=` only, keeps template inline (exceeds printWidth)

- `Inline` — multibyte quasi fits on one line (89 visual cols), both keep inline
- `Fits` — after `=` break, continuation is exactly 100 visual cols, both keep inline
- `Over` — after `=` break, continuation is 101 visual cols, Prettier inline, tsv breaks at `${`

## Reason

Print width. The threshold counts rendered columns, not source bytes: 33 CJK chars are 99 bytes but only 66 columns, so a byte-based threshold would break a template that visually fits. tsv applies the same printWidth rules after `=` breaks — consistent with template literal value handling.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §TypeScript: Template Literals.

## Related

- [template_literal_type_long](../template_literal_type_long_prettier_divergence/) — ASCII boundary cases (same divergence)
- [template_literal_type_conditional_long](../template_literal_type_conditional_long_prettier_divergence/) — conditional types break at `?`/`:` instead of `${`
