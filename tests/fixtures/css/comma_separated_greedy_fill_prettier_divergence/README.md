# comma_separated_greedy_fill_prettier_divergence

Prettier's `fill()` allows comma-separated CSS values to exceed printWidth by 1 char when trailing punctuation (`;`) is added after greedy packing.

tsv: wraps before exceeding 100 chars
Prettier: allows 101 chars (greedy fill + trailing `;`)

The root cause is Prettier's `fill()` packing to exactly the remaining width (`>= 0` check), then the parent adding `;` which pushes past the limit.

## Reason

See [conformance_prettier.md §CSS: Layout](../../../../docs/conformance_prettier.md#css-layout) (`Greedy fill overflow`, Print width). tsv treats printWidth as a hard limit. This affects all comma-separated CSS values (`animation-name`, `font-family`, `background-image`, etc.).

## Related

- [comma_space_separated_long](../values/lists/comma_space_separated_long_prettier_divergence/) · [space_separated_long_wrap](../values/lists/space_separated_long_wrap_prettier_divergence/) — sibling fill-boundary print-width divergences
