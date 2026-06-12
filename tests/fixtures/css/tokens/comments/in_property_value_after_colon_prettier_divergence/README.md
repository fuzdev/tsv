# in_property_value_after_colon_prettier_divergence

Prettier preserves varying whitespace after `:` when a comment follows (`font-size:/* c */`, `font-size: /* c */`, `font-size:  /* c */`).

tsv: normalizes to single space after `:`
Prettier: preserves whatever spacing the input has

## Reason

tsv normalizes whitespace around comments consistently. Consistent with tsv's handling across all CSS comment spacing contexts.

## Related

- [in_property_value_before_colon](../in_property_value_before_colon_prettier_divergence/) — same pattern before `:`
