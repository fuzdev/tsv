# highlight_comment_prettier_divergence

Comments inside a `::highlight()` name argument are preserved on both sides of the
value — before it (`::highlight(/* c */ name1)`) and after it
(`::highlight(name2 /* c */)`). The argument parses as an ordinary selector list
(Svelte's model), so a gap comment leads or trails the list exactly like `:is()`;
both edge positions are valid and accepted by parseCss.

`::highlight()` shares one parser path with the `:dir()` / `:lang()` args — the
strict selector-list grammar `:not()` / `:global()` use (see [dir_lang_comment](../../pseudo_class/dir_lang_comment_prettier_divergence/)).

## Prettier divergence

Gap-comment spacing normalizes to a single space, keeping the comment's side —
the same rule as every other selector-comment position — while prettier
preserves the source spacing. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded forms that prettier keeps stable; tsv
normalizes both to `input.svelte`.

## Reason

Stable quirk. tsv registers these gap comments at parse time and re-emits them
through `comments_in_range`, so the spacing normalizes uniformly — the same doc
path used for `::part()`/`::slotted()`/`:is()` argument comments. Prettier
preserves the source whitespace instead. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [dir_lang_comment](../../pseudo_class/dir_lang_comment_prettier_divergence/) — the `:dir()`/`:lang()` selector-list args (same parser path, same normalization)
- [part_comment](../part_comment_prettier_divergence/) — the `::part()` identifier arg (same normalization)
- [slotted_comment](../slotted_comment_prettier_divergence/) — the `::slotted()` compound arg (same normalization)
