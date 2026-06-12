# template_operand_long_prettier_divergence

Prettier has two stable forms when a binary operator's template operand exceeds print_width: broken `${` (tsv's form) and inline `${expr}` (Prettier's quirk form). Both are idempotent under Prettier.

tsv: breaks at `=`, `&&`, AND `${` — normalizes all inputs to broken form
Prettier: breaks at `=` and `&&`, keeps template inline (101+ chars)

## Reason

tsv breaks template interpolations to respect print_width. Consistent with tsv's template literal handling across all contexts.
