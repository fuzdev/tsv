# part_comment_prettier_divergence

Comments inside `::part()` arguments are preserved on both sides of the
identifier list — before it (`::part(/* c */ name1)`) and after it
(`::part(name2 /* c */)`). These edge positions are accepted by parseCss. A
comment *between* two identifiers (`::part(a /* c */ b)`) is instead rejected by
parseCss (it reads as whitespace, splitting the identifier run); tsv preserves
that one too, normalized the same way, as a `_svelte_prettier_divergence` — see
[part_interior_comment](../part_interior_comment_svelte_prettier_divergence/).

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

- [slotted_comment](../slotted_comment_prettier_divergence/) — the `::slotted()` compound arg (same normalization)
- [nth_comment](../../pseudo_class/nth_comment_svelte_prettier_divergence/) — `:nth-*()` argument comments
- [selector_nested_comment](../../../tokens/comments/selector_nested_comment_prettier_divergence/) — the same single-space normalization inside `:is()` args
