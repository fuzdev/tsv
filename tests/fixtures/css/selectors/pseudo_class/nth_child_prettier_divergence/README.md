# nth_child_prettier_divergence

Prettier inconsistently normalizes An+B notation: spaces `+` operators (`2n+1` -> `2n + 1`) but not `-` operators (`3n-2` stays `3n-2`).

tsv: `li:nth-child(3n - 2)` (both operators spaced consistently)
Prettier: `li:nth-child(3n-2)` (minus not normalized)

## Reason

Per CSS Syntax Module Level 3, whitespace between An+B tokens is valid and ignored — both forms are semantically identical. tsv normalizes both operators consistently.
