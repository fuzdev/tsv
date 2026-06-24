# space_separated_long_wrap_prettier_divergence

When a CSS space-separated value exceeds printWidth, Prettier keeps it on one line. tsv wraps.

tsv: wraps to respect printWidth
Prettier: allows lines to exceed printWidth (101+ chars)

## Reason

Print width. tsv treats printWidth as a hard limit and breaks a space-separated value that exceeds 100 chars; Prettier leaves it on one line. See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values) ("Space-separated value wrap").

## Related

- [transform_long](../../functions/transform_long_prettier_divergence/) — same pattern for function-heavy values
- [comma_space_separated_long](../comma_space_separated_long_prettier_divergence/) — comma + space variant
