# unknown_arg_comment_prettier_divergence

Comments inside an unrecognized functional pseudo-class's selector argument are
preserved on both sides — before it (`:current(/* c */ .class1)`) and after it
(`:current(.class2 /* c */)`). A pseudo-class outside the known set (`:is`,
`:not`, `:nth-*`, `:dir`/`:lang`, …) parses its argument as a selector list, the
same grammar `:is()` uses.

## Prettier divergence

Gap-comment spacing normalizes to a single space, keeping the comment's side —
the same rule as every other selector-comment position — while prettier
preserves the source spacing. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded forms that prettier keeps stable; tsv
normalizes both to `input.svelte`.

## Reason

Stable quirk. tsv registers these gap comments at parse time and re-emits them
through `comments_in_range` — the identical selector-list doc path `:is()` uses,
so the fix is parser-only. Prettier preserves the source whitespace instead. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_nested_comment](../../../tokens/comments/selector_nested_comment_prettier_divergence/) — the same single-space normalization inside `:is()` args
- [nth_comment](../nth_comment_svelte_prettier_divergence/) — `:nth-*()` argument comments
