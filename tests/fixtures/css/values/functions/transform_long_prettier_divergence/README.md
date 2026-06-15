# transform_long_prettier_divergence

Prettier doesn't wrap space-separated CSS value lists at the printWidth boundary.

tsv: wraps at 101 chars (>100)
Prettier: wraps at 102 chars (off-by-one)

This affects all space-separated CSS value lists — `transform`, `filter`, `box-shadow`, `text-shadow`, and any property with space-separated function lists.

## Reason

tsv enforces consistent wrapping at 100 chars for all CSS value lists. Prettier's off-by-one creates inconsistency with other constructs.

## Related

- [supports_long](../../at_rules/supports_long_prettier_divergence/) — similar off-by-two
- [space_separated_long_wrap](../../values/lists/space_separated_long_wrap_prettier_divergence/) — single value variant
