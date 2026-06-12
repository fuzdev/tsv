# comma_space_separated_long_prettier_divergence

At 101 chars, Prettier tolerates the overage and keeps space-separated values inline. tsv breaks to stay within 100 chars.

tsv: wraps space-separated values within comma-separated list items
Prettier: allows 101 char lines (1 over print_width)

## Reason

tsv treats print_width as a hard limit. At 100 and 102 chars both formatters match — this divergence only manifests at the 101-char boundary.

## Related

- [comma_separated_greedy_fill](../../comma_separated_greedy_fill_prettier_divergence/) — same fill boundary pattern
- [space_separated_long_wrap](../space_separated_long_wrap_prettier_divergence/) — single value variant
