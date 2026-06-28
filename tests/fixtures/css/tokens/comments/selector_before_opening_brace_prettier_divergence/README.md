# selector_before_opening_brace_prettier_divergence

Prettier preserves varying whitespace between selector and comment before `{` (`.class/* c */`, `.class /* c */`, `.class  /* c */`, and a newline before the comment).

tsv: normalizes to single space (newline included)
Prettier: preserves whatever spacing the input has

## Reason

Stable quirk. tsv normalizes comment spacing consistently across the CSS contexts whose grammar it parses (selectors, `@media`/`@supports` preludes, declaration values); prettier preserves whatever spacing the source has. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_before_opening_brace_in_atrule](../selector_before_opening_brace_in_atrule_prettier_divergence/) — the same divergence for a rule nested in an at-rule
- [atrule_before_opening_brace](../atrule_before_opening_brace_prettier_divergence/) — same pattern for at-rules
- [selector_list](../selector_list_prettier_divergence/) — comment spacing in selector lists
