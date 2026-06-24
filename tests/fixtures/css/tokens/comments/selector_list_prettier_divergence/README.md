# selector_list_prettier_divergence

Prettier preserves varying whitespace after comma in selector lists with comments (`.class1,/* c */`, `.class1, /* c */`, `.class1,  /* c */`).

tsv: normalizes to single space after comma
Prettier: preserves whatever spacing the input has

## Reason

Stable quirk. tsv normalizes comment spacing consistently across the CSS contexts whose grammar it parses; prettier preserves whatever spacing the source has after the comma. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/) — comment spacing before `{`
