# transform_long_prettier_divergence

Prettier doesn't wrap space-separated CSS value lists at the printWidth boundary.

tsv: wraps at 101 chars (>100)
Prettier: wraps at 102 chars (off-by-one)

This affects all space-separated CSS value lists — `transform`, `filter`, `box-shadow`, `text-shadow`, and any property with space-separated function lists.

## Reason

Print width. tsv treats printWidth as a hard limit and wraps all CSS space-separated value lists at 100 chars; Prettier's off-by-one tolerates the overage. See [conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values) ("Transform list wrap").

## Related

- [supports_long](../../at_rules/supports_long_prettier_divergence/) — similar off-by-two
- [space_separated_long_wrap](../../values/lists/space_separated_long_wrap_prettier_divergence/) — single value variant
