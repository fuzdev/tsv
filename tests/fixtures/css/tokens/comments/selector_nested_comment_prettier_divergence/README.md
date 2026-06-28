# selector_nested_comment_prettier_divergence

Selector-comment spacing normalizes consistently in positions the old string-replace
seam couldn't reach: inside a forgiving list (`:is()`/`:where()`/`:not()`/`:has()`) and
when a comment sits **before** the comma.

tsv: normalizes to a single space around the comma boundary, preserving the comment's
side (`:is(.a, /* mid */ .b)`, `.c /* before */, .d`)
Prettier: preserves whatever spacing the source has (`:is(.a,/* mid */.b)`, `.c/* before */,.d`)

## Reason

Stable quirk. tsv interleaves selector comments at comma boundaries through the same
doc path it uses for the rest of the selector (registered at parse time, emitted via
`comments_in_range`), so the spacing normalizes uniformly at every nesting level; prettier
preserves the source whitespace. This is the principled successor to the top-level
[selector_list](../selector_list_prettier_divergence/) divergence — same rule, now applied
inside `:is()`/attribute selectors and to before-comma comments. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_list](../selector_list_prettier_divergence/) — the top-level comma case (same normalization)
- [selector_before_opening_brace](../selector_before_opening_brace_prettier_divergence/) — comment spacing before `{`
