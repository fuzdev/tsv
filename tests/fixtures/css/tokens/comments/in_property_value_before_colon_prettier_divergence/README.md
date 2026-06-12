# in_property_value_before_colon_prettier_divergence

Prettier omits spaces around comments before colons and preserves arbitrary whitespace variants.

tsv: `color /* comment */ : red;` (normalized single spaces)
Prettier: `color/* comment */: red;` (no spaces around comment)

Prettier also preserves compact (`color/* comment */:red;`) and extra-spaced (`color  /* comment */  :  red;`) forms verbatim.

## Reason

tsv normalizes comment spacing consistently. Consistent with tsv's handling across all CSS comment spacing contexts.

## Related

- [in_property_value_after_colon](../in_property_value_after_colon_prettier_divergence/) — same pattern after `:`
