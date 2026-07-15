# fill_competing_expr_prettier_divergence

An inline element whose fill content holds **two** breakable expressions and overflows. Only one of
them has to break for the content to fit, so the layout has a genuine choice — and the sibling
[fill_multiple_expr_long](../fill_multiple_expr_long_prettier_divergence/), whose single ternary
must break, cannot exercise it.

tsv breaks the **first** expression at its `?`/`:` and leaves the second flat, holding every line
inside 100 columns. Prettier keeps the first flat and, having nothing left to give, breaks the
second at its `!==` operator — landing the head line at 102 characters.

## Reason

Print width. tsv treats printWidth as a hard limit and prefers breaking a ternary at `?` over
breaking a comparison at `!==`.

## Why the choice must be stable

The element's content boundary carries whitespace, so it takes the padded (non-hugging) boundary
path. The expression groups have to stay breakable there. If they don't, the fill's `line`
separators short-circuit the width check of the expression group before them — each pass leaves a
*different* expression flat, so the two layouts alternate forever and the format never settles.
`unformatted_ours_hug` is the same document authored with the content hugging the opening tag; it
must converge on `input` rather than starting that oscillation. Prettier reads that hugged boundary
as an instruction and dangles the opening delimiter instead (`prettier_variant_dangle`, which tsv
likewise normalizes to `input`) — see
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
