# inline_content_hug_long_prettier_divergence

When inline element content with expressions exceeds print_width, there are two strategies:

1. Break opening bracket: `<small\n\t>content</small\n>` (Prettier)
2. Break expression internally: `<small>{expr\n\t? x\n\t: y}</small\n>` (tsv)

tsv: breaks expressions (ternaries at `?`, logical chains at `&&`, binaries at `+`)
Prettier: breaks the opening bracket

Applies to ternaries, logical chains, and binary expression chains. At 100 chars both formatters keep inline. At 101 chars both break only the closing `>`. At 102+ chars with breakable expressions, tsv breaks the expression while Prettier breaks the bracket.

When expressions are unbreakable identifiers, both formatters break at the bracket identically.

## Reason

Breaking expressions saves 1 tab indent level per nesting depth, keeps `<tag>{content}` visually connected, and follows operator precedence for break points. In real-world cases (e.g., ThreadListitem.svelte), tsv produces max 84-char lines vs Prettier's 108 chars (exceeding by 8).
