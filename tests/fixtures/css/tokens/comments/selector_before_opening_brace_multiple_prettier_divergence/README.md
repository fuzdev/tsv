# selector_before_opening_brace_multiple_prettier_divergence

Prettier preserves varying whitespace between a selector and two or more comments before `{` (`.class/* a *//* b */`, `.class /* a */ /* b */`). tsv normalizes spacing.

tsv: `.class /* a */ /* b */ {` (single space, `{` inline)
Prettier: preserves the input spacing, and for the no-space (compact) form with ≥2 comments drops `{` onto its own line (`.class/* a *//* b */\n{`)

## Reason

Stable quirk. tsv normalizes comment spacing consistently across the CSS contexts whose grammar it parses. Same quirk as the single-comment case; the compact `{`-on-its-own-line form is specific to multiple pre-brace comments. See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/) — the single-comment case (same normalization)
- [atrule_before_opening_brace](../atrule_before_opening_brace_prettier_divergence/) — same pattern for at-rules
