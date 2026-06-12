# inline_element_fill_long_prettier_divergence

At the fill boundary with an inline element, Prettier allows 101 chars (1 over) while tsv breaks one word earlier (94 chars).

tsv: breaks at 94 chars (strict print_width)
Prettier: allows 101 chars (emergent from `group([line, element])` wrappers)

At 100 chars both formatters match — see `inline_element_fill_100/`.

## Reason

prettier-plugin-svelte structures docs with separate fills per text node and `group([line, element])` wrappers around inline elements. This creates an emergent behavior where lines can exceed print_width by 1 char. tsv's fill strictly respects print_width=100.
