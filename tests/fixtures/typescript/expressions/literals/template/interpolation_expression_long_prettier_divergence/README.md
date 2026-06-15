# interpolation_expression_long_prettier_divergence

When a template literal interpolation `${...}` contains a long expression exceeding printWidth, Prettier keeps it inline. tsv breaks after `${` and puts `}` on its own line.

tsv: `${\n\tobj.aaa.bbb.ccc...\n}` (respects printWidth)
Prettier: `${obj.aaa.bbb.ccc...}` (exceeds printWidth)

## Reason

tsv treats printWidth as a hard limit for template interpolations. Consistent with tsv's template literal handling across all contexts.
