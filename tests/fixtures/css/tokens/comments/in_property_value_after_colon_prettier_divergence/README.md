# in_property_value_after_colon_prettier_divergence

Prettier preserves varying whitespace after `:` when a comment follows (`font-size:/* c */`, `font-size: /* c */`, `font-size:  /* c */`).

tsv: normalizes to single space after `:`
Prettier: preserves whatever spacing the input has

That holds for a **newline** too (`prettier_variant_newline`), and there the consequence is
sharper than spacing: postcss keeps the whole property→value gap in `raws.between`, which
prettier emits verbatim, so the authored *indentation* comes through with it — one stable
output per authored indent, including indents that put the value left of its own property.
tsv normalizes the gap and so has a single fixed point.

## Reason

Stable quirk. tsv normalizes whitespace around comments consistently. Consistent with tsv's handling across all CSS comment spacing contexts. See [conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [in_property_value_before_colon](../in_property_value_before_colon_prettier_divergence/) — same pattern before `:`
