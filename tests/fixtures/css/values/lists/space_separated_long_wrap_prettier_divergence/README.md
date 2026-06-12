# space_separated_long_wrap_prettier_divergence

When a CSS space-separated value exceeds print_width, Prettier keeps it on one line. tsv wraps.

tsv: wraps to respect print_width
Prettier: allows lines to exceed print_width (101+ chars)

## Reason

tsv treats print_width as a hard limit. This appears to be a limitation in prettier-plugin-svelte's CSS handling rather than an intentional design choice.

## Related

- [transform_long](../../functions/transform_long_prettier_divergence/) — same pattern for function-heavy values
- [comma_space_separated_long](../comma_space_separated_long_prettier_divergence/) — comma + space variant
