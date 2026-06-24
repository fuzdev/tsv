# selector_before_opening_brace_prettier_divergence

Prettier preserves varying whitespace between selector and comment before `{` (`.class/* c */`, `.class /* c */`, `.class  /* c */`).

tsv: normalizes to single space
Prettier: preserves whatever spacing the input has

## Reason

Stable quirk. tsv normalizes comment spacing consistently across the CSS contexts whose grammar it parses (selectors, `@media`/`@supports` preludes, declaration values); prettier preserves whatever spacing the source has. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [atrule_before_opening_brace](../atrule_before_opening_brace_prettier_divergence/) — same pattern for at-rules
- [selector_list](../selector_list_prettier_divergence/) — comment spacing in selector lists
