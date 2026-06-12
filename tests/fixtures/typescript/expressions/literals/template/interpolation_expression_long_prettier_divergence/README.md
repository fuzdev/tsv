# interpolation_expression_long_prettier_divergence

When a template literal interpolation `${...}` contains a long expression exceeding print_width, Prettier keeps it inline. tsv breaks after `${` and puts `}` on its own line.

tsv: `${\n\tobj.aaa.bbb.ccc...\n}` (respects print_width)
Prettier: `${obj.aaa.bbb.ccc...}` (exceeds print_width)

## Reason

tsv treats print_width as a hard limit for template interpolations. Consistent with tsv's template literal handling across all contexts.
