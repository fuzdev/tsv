# nth_comment_svelte_prettier_divergence

Comments inside `:nth-*()` arguments are preserved in every position: before the
An+B, inside the An+B expression text (after the expression, or between it and
`of`), between `of` and the selector list, and after the `of` selector list.

## Svelte divergence

Svelte's `parseCss` rejects a comment in nth args everywhere except before the
An+B (`css_expected_identifier` — its An+B scanner doesn't tokenize comments), so
`expected_svelte.json` records the parse error. tsv accepts them all: per CSS
Syntax 3, comments are valid wherever whitespace is. See
[conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

Gap-comment spacing normalizes to single spaces, keeping the comment's side —
the same rule as every other selector-comment position — while prettier
preserves the source spacing. The gaps are: before the An+B, inside the An+B
expression text, between `of` and the selector list, and after the `of` selector
list; `prettier_variant_compact` pins the glued forms prettier keeps stable.

A comment **inside** the An+B expression text does not freeze it — the operator
spacing normalizes as usual, and the comment is opaque to the respacing, so its
content is never rewritten (`/* a-b */` must not become `/* a - b */`). Prettier
prints the *whole selector* verbatim whenever it holds a comment, which is its
selector parser giving up rather than a rule about An+B, so
`prettier_variant_compact` pins the glued `2n+1 /* a-b */` spelling it keeps. See
[conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [nth_child_of](../nth_child_of_svelte_prettier_divergence/) — the `of S` AST structure + `of` spacing
- [selector_nested_comment](../../../tokens/comments/selector_nested_comment_prettier_divergence/) — the same single-space normalization inside `:is()` args
