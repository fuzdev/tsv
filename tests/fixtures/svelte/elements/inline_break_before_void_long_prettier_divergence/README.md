# inline_break_before_void_long_prettier_divergence

Void / self-closing parity for the break-before rule (see `inline_break_before_wrap_long`).
A void element (`<img />`) preceded by same-line text moves to its **own line** rather than
dangling its opening tag at the end of the text line; with no content it simply sits on the
fresh line.

tsv: the opening tag never dangles after a space — the whole element moves to its own line.
Prettier: keeps the opening tag on the text line and wraps its attributes — see
`prettier_variant_dangle.svelte` (prettier keeps that form; tsv normalizes it to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring (tsv → `input.svelte`, prettier → dangle).

## Reason

Design choice, render-free under Svelte 5 (the whitespace boundary before the element
collapses to one space at compile, so the break is render-equivalent).
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
