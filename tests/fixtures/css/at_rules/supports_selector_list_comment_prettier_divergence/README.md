# supports_selector_list_comment_prettier_divergence

A comment beside the comma of a `selector()` list keeps its side of the comma — before it, the comment trails the selector; after it, the comment leads the next — with the whitespace around it normalized to single spaces. Prettier keeps the comment in place too, but freezes whatever spacing the source has around it.

tsv: `selector(.class1, /* c1 */ .class2)` (normalized)
Prettier: `selector(.class1,/* c1 */.class2)` (glued stays glued)
Prettier: `selector(.class1,   /* c1 */   .class2)` (padded stays padded)

The same seam, the same rule and the same quirk as a rule's own selector list ([selector_list](../../tokens/comments/selector_list_prettier_divergence/)) — the argument prints through the selector printer's comma seam, in `@supports` and in an `@import` prelude's `supports()` alike, beside a condition connector, ahead of a selector with a combinator, and for a run of comments (joined single-spaced, like every other selector gap).

## Reason

Stable quirk. tsv normalizes comment spacing consistently across the CSS contexts whose grammar it parses; prettier preserves whatever spacing the source has around the comment. See [conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [selector_list](../../tokens/comments/selector_list_prettier_divergence/) — the same seam in a rule's selector list
- [supports_selector_comment](../supports_selector_comment/) — comments at the other positions of a `selector()` argument
- [supports_selector_argument](../supports_selector_argument_prettier_divergence/) — a comment-free list, where prettier breaks one selector per line
