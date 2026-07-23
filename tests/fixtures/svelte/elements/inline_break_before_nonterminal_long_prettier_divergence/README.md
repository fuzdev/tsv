# inline_break_before_nonterminal_long_prettier_divergence

The break-before rule (see `inline_break_before_wrap_long`) with a following
whitespace-separated sibling — i.e. the element is **not** at the tail of the fill. The `<a>`
is preceded by same-line text and followed by ` tail`; its content is too long to collapse, so
it breaks to a fresh line and lays out block-style. The terminal trailing text ` tail` hugs the
intact `</a>` (the existing terminal-hug behavior).

tsv: `<a>` starts a fresh line; the opening tag never dangles after a space.
Prettier: keeps the opening tag on the text line and dangles it — see
`prettier_variant_dangle.svelte` (prettier keeps that form; tsv normalizes it to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring (tsv → `input.svelte`, prettier → dangle).

The boundary before `<a>` is inter-node whitespace (render-free under Svelte 5), so the break is
render-equivalent.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
