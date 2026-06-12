# nth_child_of_svelte_divergence

Svelte parser bug: incorrectly includes `"of "` in the Nth value and treats the selector list as siblings instead of nesting it.

tsv: `Nth.value = "2n"`, with `Nth.selector` containing the nested selector list (spec-compliant)
Svelte: `Nth.value = "2n of "`, with selectors as siblings of the Nth node (incorrect)

## Reason

CSS Selectors Level 4 (`#the-nth-child-pseudo`) defines `:nth-child(An+B [of S]?)` where `S` is a `<<complex-real-selector-list>>` nested inside the pseudo-class arguments. Svelte's parser doesn't separate the `of` keyword from the An+B notation, corrupting both the value and the AST structure.

Applies to both `:nth-child()` and `:nth-last-child()`. Does not apply to `:nth-of-type()` or `:nth-last-of-type()` (which don't support `of S`).

## Fixture Structure

- `expected_ours.json` — tsv's spec-compliant output (source of truth)
- `expected_svelte.json` — documents Svelte's incorrect output
