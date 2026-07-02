# nth_child_of_svelte_prettier_divergence

Svelte parser bug: incorrectly includes `"of "` in the Nth value and treats the selector list as siblings instead of nesting it.

tsv: `Nth.value = "2n"`, with `Nth.selector` containing the nested selector list (spec-compliant)
Svelte: `Nth.value = "2n of "`, with selectors as siblings of the Nth node (incorrect)

## Reason

CSS Selectors Level 4 (`#the-nth-child-pseudo`) defines `:nth-child(An+B [of S]?)` where `S` is a `<<complex-real-selector-list>>` nested inside the pseudo-class arguments. Svelte's parser doesn't separate the `of` keyword from the An+B notation, corrupting both the value and the AST structure.

Applies to both `:nth-child()` and `:nth-last-child()`. Does not apply to `:nth-of-type()` or `:nth-last-of-type()` (which don't support `of S`).

## Prettier divergence

tsv always emits single spaces around the `of` keyword; prettier collapses
whitespace runs there but never inserts an absent space, so the glued
`of.class1` form is prettier-stable (`prettier_variant_of_compact` pins it;
gluing the other side, `2n of`/`of div`, would merge the idents, so only the
`of.class` side can glue). Svelte's `parseCss` also rejects the glued form
(`css_expected_identifier`), but only `input.svelte` is parse-checked. See
[conformance_prettier.md §CSS: Selectors](../../../../../../docs/conformance_prettier.md#css-selectors).

## Fixture Structure

- `expected_ours.json` — tsv's spec-compliant output (source of truth)
- `expected_svelte.json` — documents Svelte's incorrect output

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).
