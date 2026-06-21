# inline_nested_child_trailing_space_long_prettier_divergence

The space-authored companion of `inline_nested_child_trailing_long`. An inline `<span>` wraps a
wide inline child `<span>` (its open tag overflows print width) plus trailing text. tsv drops the
child below the parent's open tag and puts the trailing text on its **own line**. Both the
newline-authored boundary (the `input` shape) and the space-authored boundary
(`unformatted_ours_compact`) currently converge to this one form. (This differs from the terminal
case `inline_wide_content_trailing_long`, which now hugs a space-authored tail; aligning the
nested-child case is tracked consistency work.)

Prettier keeps the child on its own line too, but **hugs the trailing text** after the child's
closing tag (`</span> text`) — see `prettier_variant_hug.svelte` (prettier's stable form, which tsv
normalizes back to `input.svelte`).

tsv: child drops below the parent open tag, trailing text on its own line
Prettier: child drops below the parent open tag, trailing text hugs the child's closing `>`

This pins the **authoring-independence** of the boundary: feeding the space-authored form must reach
the same fixed point as the newline-authored form. Before the fix the space form was
non-idempotent — the parent `<span>`'s `>` dangled on its own line on the first pass and collapsed on
the second, settling on a second fixed point.

## Reason

tsv treats printWidth as a hard limit and a wide inline child that does not fit drops to its own
line whole; trailing text after such a dropped child currently takes its own line rather than
hugging the child's closing `>` (a known divergence from the terminal-text behavior, pending
alignment). See
[conformance_prettier.md §Wide inline content + trailing text](../../../../../docs/conformance_prettier.md#svelte-elements).
