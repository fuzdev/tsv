# selector_before_opening_brace_in_atrule_prettier_divergence

The nested form of [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/):
a rule whose pre-`{` selector comment lives inside an at-rule block. Prettier
preserves whatever whitespace the source has between the selector and the
comment — including a newline (`.class⏎/* comment */ {`).

tsv: normalizes the whitespace to a single space (`.class /* comment */ {`)
Prettier: preserves the source whitespace, newline included

## Reason

Stable quirk. tsv normalizes pre-`{` selector-comment whitespace consistently,
and that normalization is the same at every nesting level — a rule nested
directly in an at-rule prints through the same rule printer as a top-level
rule. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/) — the top-level form
- [atrule_before_opening_brace](../atrule_before_opening_brace_prettier_divergence/) — same pattern for at-rules
