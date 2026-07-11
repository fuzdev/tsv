# scope_selector_comment_prettier_divergence

Comments inside an `@scope` prelude's root or limit selector list are preserved
on both sides — leading it (`@scope (/* c */ .a)`) and trailing it
(`@scope (.a /* c */)`), in either the root `(…)` or the limit `to (…)` clause.
Both clauses parse through the same complex-selector-list path `:is()` uses, so
the same selector-comment machinery re-emits them.

## Prettier divergence

Gap-comment spacing normalizes to a single space, keeping the comment's side —
the same rule as every other selector-comment position — while prettier
preserves the source spacing. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded forms that prettier keeps stable; tsv
normalizes both to `input.svelte`.

## Reason

Stable quirk. tsv registers these gap comments at parse time and re-emits them
through `comments_in_range` — the identical selector-comment path `:is()` uses.
Prettier preserves the source whitespace instead. parseCss accepts the input and
strips the comment from the wire `prelude` string (`( .a) to (.b )`), so this is
a prettier-only divergence. See
[conformance_prettier.md §CSS: Comments](../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [unknown_arg_comment](../../selectors/pseudo_class/unknown_arg_comment_prettier_divergence/) — the same single-space normalization inside an unrecognized pseudo-class's selector arg
- [scope_selector](../scope_selector_prettier_divergence/) — `@scope` selector-list whitespace normalization (no comments)
