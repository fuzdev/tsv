# inline_break_before_glued_long_prettier_divergence

The break-before-an-inline-element rule (see `inline_break_before_wrap_long`) breaks at the
last **whitespace** boundary before the element — never between glued text and the element.
When an inline element is glued (no whitespace) to preceding text, that text travels *with*
the element to the fresh line. Here `glued` is glued to the `<a>`, so the break lands before
`glued` and `glued<a …>content</a>.` moves to the fresh line as one unit.

Breaking between `glued` and `<a>` would be **render-changing** — the glued boundary is
render-significant (it injects a rendered space, so the text data would gain a trailing
space). tsv only ever breaks at the whitespace boundary before the glued run, which is
render-equivalent (confirmed by `ast_diff --render`).

tsv: `glued<a>` stays glued and moves to its own line together.
Prettier: keeps the glued run on the text line and dangles the `<a>` closing tag — see
`output_prettier.svelte` (prettier's stable form). `unformatted_ours_compact.svelte` is the
compact authoring (tsv → `input.svelte`, prettier → `output_prettier.svelte`).

## Reason

Design choice, render-free under Svelte 5 for the *whitespace* boundary; render-significant
for the *glued* boundary, which is therefore never split.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
