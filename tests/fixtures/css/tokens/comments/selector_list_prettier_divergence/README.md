# selector_list_prettier_divergence

Prettier preserves varying whitespace after comma in selector lists with comments (`.class1,/* c */`, `.class1, /* c */`, `.class1,  /* c */`).

tsv: normalizes to single space after comma
Prettier: preserves whatever spacing the input has

## Reason

tsv normalizes comment spacing consistently across all CSS contexts.

## Related

- [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/) — comment spacing before `{`
