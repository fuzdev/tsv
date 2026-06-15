# interpolation_multiline_indent_long_prettier_divergence

When template literal interpolations exceed printWidth inside code-generation templates, tsv respects indentation using Prettier's `addAlignmentToDoc` approach: measure visual width of whitespace after the last `\n` in the quasi text before `${`, then position the expression at that absolute indent level.

tsv: breaks at `${`/`}` with proper visual alignment to quasi indentation
Prettier: keeps `${identifier` on the same line, breaks chain at dots

For inline templates (no `\n` in any quasi), alignment is skipped. Line comments inside interpolations force the group to break regardless of expression type (correctness: a line comment on a flat line would swallow `}`).

Cases 1-15 test various indent sizes (tabs, spaces, mixed). Case 16 tests deeply nested templates.

## Reason

tsv treats printWidth as a hard limit for template interpolations, with correct indentation alignment for multiline templates.
