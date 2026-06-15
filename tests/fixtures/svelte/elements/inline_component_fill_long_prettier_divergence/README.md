# inline_component_fill_long_prettier_divergence

Same fill boundary behavior as `inline_element_fill_long`, but with a component element (`<Comp>`) instead of an HTML inline element (`<a>`). At 101 chars, Prettier keeps everything on one line while tsv breaks the closing `>`.

tsv: breaks to stay within 100 chars
Prettier: allows 101 chars (1 over printWidth)

At 100 chars both formatters match.

## Reason

tsv treats printWidth as a hard limit.
