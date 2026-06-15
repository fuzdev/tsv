# fill_after_inline_prettier_divergence

Prettier's `fill()` allows lines to exceed printWidth when text follows an inline element closing tag (111 chars in this case, 11 over).

tsv: breaks at exactly 100 chars
Prettier: packs trailing text onto the line (111 chars)

## Reason

tsv treats printWidth as a hard limit. Prettier's fill algorithm doesn't account for the accumulated width when packing text after inline element wrappers.
