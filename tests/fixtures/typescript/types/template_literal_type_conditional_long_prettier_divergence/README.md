# template_literal_type_conditional_long_prettier_divergence

When a conditional type inside a template literal type exceeds printWidth, Prettier preserves whatever form is given (stable variant — both compact and broken are idempotent). tsv normalizes to break at `?` and `:` operators.

tsv: breaks at `?` and `:` at 101+ chars
Prettier: preserves input form (stable variant, allows inline at 101+ chars)

| Line width | tsv                   | Prettier                         |
| ---------- | --------------------- | -------------------------------- |
| 100 chars  | inline                | inline                           |
| 101+ chars | breaks at `?` and `:` | preserves input (stable variant) |

Tests function return types and type aliases.

## Reason

Conditional types have natural break points at `?` and `:` that show the ternary structure clearly, consistent with how conditional types format elsewhere in TypeScript.

## Related

- [template_literal_type_long](../template_literal_type_long_prettier_divergence/) — simple type references break at `${` instead
