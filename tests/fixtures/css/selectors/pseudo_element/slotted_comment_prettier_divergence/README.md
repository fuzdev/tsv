# slotted_comment_prettier_divergence

Comments inside `::slotted()` arguments are preserved on both sides of the
compound selector — before it (`::slotted(/* c */ div)`) and after it
(`::slotted(div /* c */)`). Only the edge positions are valid: a comment
*between* compound parts (`::slotted(div /* c */ .foo)`) is rejected by both
parsers (a comment reads as whitespace, which a compound selector forbids).

## Prettier divergence

Gap-comment spacing normalizes to a single space, keeping the comment's side —
the same rule as every other selector-comment position — while prettier
preserves the source spacing. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded forms that prettier keeps stable; tsv
normalizes both to `input.svelte`.

## Reason

Stable quirk. tsv registers these gap comments at parse time and re-emits them
through `comments_in_range`, so the spacing normalizes uniformly — the same doc
path used for `:is()`/`:nth-*()` argument comments. Prettier preserves the source
whitespace instead. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [part_comment](../part_comment_prettier_divergence/) — the `::part()` identifier arg (same normalization)
- [nth_comment](../../pseudo_class/nth_comment_svelte_prettier_divergence/) — `:nth-*()` argument comments
- [selector_nested_comment](../../../tokens/comments/selector_nested_comment_prettier_divergence/) — the same single-space normalization inside `:is()` args
