# inline_break_before_wrap_long_prettier_divergence

An inline element preceded by same-line inline content that must wrap starts on a **fresh
line**. tsv breaks at the whitespace boundary before the element rather than dangling its
opening tag at the end of the text line. On its fresh line the element then lays out like
any other: it collapses back inline when it fits, else goes block-style (both tags intact,
content on its own indented line) or wraps its attributes.

Cases (in order):

1. **Fits at exactly 100** — element stays inline on the text line, no break (control).
2. **101** — one char longer, so the element breaks to a fresh line and collapses there.
3. **Last child** — no trailing content; same fresh-line collapse.
4. **Content too long to collapse** — break before, then block-style.
5. **Opening tag over 100** — break before, then attributes wrap from the fresh line.

tsv: the opening tag never dangles after a space — the element (with any glued prefix)
moves to its own line, every line ≤100.

Prettier: keeps the opening tag on the text line and **dangles** it (`</a` / `>` split
across lines), letting the line run past printWidth — see `prettier_variant_dangle.svelte`
(prettier keeps that form stable; tsv normalizes it back to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring both formatters start from:
tsv normalizes it to `input.svelte`, prettier normalizes it to the dangle form.

## Reason

Design choice, render-free under Svelte 5. The whitespace boundary before an inline element
is inter-node whitespace that collapses to a single space at compile, so turning it into a
line break is render-equivalent (confirmed by `ast_diff --render`). tsv treats a wrapping
inline element uniformly with block elements — it starts a fresh line rather than leaving an
opening tag stranded at the end of the preceding text line — where prettier lets the
authored boundary decide and dangles the tag delimiters.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
