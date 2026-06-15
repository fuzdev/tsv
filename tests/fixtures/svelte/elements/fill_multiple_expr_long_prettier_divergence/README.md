# fill_multiple_expr_long_prettier_divergence

When fill content with multiple expression tags exceeds printWidth, Prettier breaks the opening bracket while tsv keeps the bracket hugging and breaks ternary expressions instead.

tsv: `<small>{expr} text{expr !== 1\n\t? 's'\n\t: ''}` (max 79 chars)
Prettier: `<small\n\t>{expr} text{expr !== 1 ? 's' : ''}` (101 chars, exceeds)

## Reason

tsv treats printWidth as a hard limit. Breaking ternaries at `?` is more readable than breaking at `!==`, and keeping `<tag>{content}` on one line provides cleaner structure.
