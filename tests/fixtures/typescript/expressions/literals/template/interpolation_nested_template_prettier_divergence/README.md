# interpolation_nested_template_prettier_divergence

When a template literal interpolation contains another template literal and exceeds printWidth, Prettier keeps `${...}` inline. tsv breaks at `${`/`}` boundaries.

tsv: breaks at `${`/`}` when content exceeds printWidth
Prettier: keeps `${...}` inline (exceeds printWidth)

Tests various expression contexts: ternaries, logical operators, arrays, function calls, and arrow callbacks within nested templates.

## Reason

tsv treats printWidth as a hard limit for template interpolations. Consistent with tsv's template literal handling across all contexts.
