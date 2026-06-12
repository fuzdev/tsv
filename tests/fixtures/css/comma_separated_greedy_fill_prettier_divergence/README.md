# comma_separated_greedy_fill_prettier_divergence

Prettier's `fill()` allows comma-separated CSS values to exceed print_width by 1 char when trailing punctuation (`;`) is added after greedy packing.

tsv: wraps before exceeding 100 chars
Prettier: allows 101 chars (greedy fill + trailing `;`)

The root cause is Prettier's `fill()` packing to exactly the remaining width (`>= 0` check), then the parent adding `;` which pushes past the limit.

## Reason

tsv treats print_width as a hard limit. This affects all comma-separated CSS values (`animation-name`, `font-family`, `background-image`, etc.).
