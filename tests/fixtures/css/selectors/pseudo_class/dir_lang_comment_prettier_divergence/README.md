# dir_lang_comment_prettier_divergence

Comments inside a `:dir()` / `:lang()` argument are preserved on both sides of the
value — before it (`:dir(/* c */ ltr)`, `:lang(/* c */ en)`) and after it
(`:dir(rtl /* c */)`, `:lang(en-US /* c */)`). These args parse as an ordinary
selector list (Svelte's model — `:lang(en, fr)` is two `TypeSelector`s), so a gap
comment leads or trails the list exactly like `:is()`; both edge positions are
valid and accepted by parseCss.

`:dir`, `:lang`, and `::highlight()` share one parser path — the strict
selector-list grammar `:not()` / `:global()` use (see [highlight_comment](../../pseudo_element/highlight_comment_prettier_divergence/)).

## Prettier divergence

Gap-comment spacing normalizes to a single space, keeping the comment's side —
the same rule as every other selector-comment position — while prettier
preserves the source spacing. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded forms that prettier keeps stable; tsv
normalizes both to `input.svelte`.

## Reason

Stable quirk. tsv registers these gap comments at parse time and re-emits them
through `comments_in_range`, so the spacing normalizes uniformly — the same doc
path used for `::part()`/`:is()`/`:nth-*()` argument comments. Prettier preserves
the source whitespace instead. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [highlight_comment](../../pseudo_element/highlight_comment_prettier_divergence/) — the `::highlight()` identifier arg (same parser path, same normalization)
- [part_comment](../../pseudo_element/part_comment_prettier_divergence/) — the `::part()` identifier arg (same normalization)
- [nth_comment](../nth_comment_svelte_prettier_divergence/) — `:nth-*()` argument comments
