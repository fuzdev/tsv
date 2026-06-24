# in_property_value_before_colon_prettier_divergence

Prettier omits spaces around comments before colons and preserves arbitrary whitespace variants.

tsv: `color /* comment */ : red;` (normalized single spaces)
Prettier: `color/* comment */ : red;` (no space before the comment)

Prettier also preserves compact (`color/* comment */:red;`) and extra-spaced (`color  /* comment */  :  red;`) forms verbatim.

## Reason

Stable quirk. tsv normalizes comment spacing consistently. Consistent with tsv's handling across all CSS comment spacing contexts. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [in_property_value_after_colon](../in_property_value_after_colon_prettier_divergence/) — same pattern after `:`
