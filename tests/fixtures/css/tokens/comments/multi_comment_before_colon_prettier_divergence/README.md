# multi_comment_before_colon_prettier_divergence

Two (or more) block comments between a property name and the `:`
(`color /* a */ /* b */ : red`). Every comment must survive formatting — the
property→colon gap is the only place a CSS declaration comment is preserved
(the parser drops value comments by design), so dropping one is silent content
loss.

tsv: `color /* a */ /* b */ : red;` (normalized single spaces; both comments
preserved)
Prettier: `color/* a */ /* b */ : red;` (no space before the first comment)

This is the same spacing divergence as
[in_property_value_before_colon](../in_property_value_before_colon_prettier_divergence/);
the added case is the second comment — reconstructing only the first drops the
rest (`color /* a */ : red;`, losing `/* b */`).

## Reason

Stable quirk. tsv normalizes comment spacing consistently and reconstructs
**all** property→colon-gap comments, joined single-spaced. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [in_property_value_before_colon](../in_property_value_before_colon_prettier_divergence/) — the single-comment case
- [colon_in_property_comment](../colon_in_property_comment_prettier_divergence/) — a comment containing a `:` (scan robustness)
