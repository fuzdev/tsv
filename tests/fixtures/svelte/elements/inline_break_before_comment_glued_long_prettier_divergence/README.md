# inline_break_before_comment_glued_long_prettier_divergence

The break-before-an-inline-element rule (see `inline_break_before_wrap_long`) reaches an
element that carries a **glued HTML-comment prefix**. When an inline element is preceded by
same-line content that must wrap and a comment is glued (no whitespace) to the element, the
comment is the element's prefix: the break lands at the last **whitespace** boundary *before*
the comment, and `<!--c--><a …>…</a>` moves to the fresh line together — never between the
comment and the element (that boundary is glued and breaking it would inject a rendered
space). This mirrors the glued *text* prefix in `inline_break_before_glued_long`; a comment
between the text and the element is just another glued prefix node.

Cases (in order):

1. **Fits at exactly 100** — comment + element stay inline on the text line, no break (control).
2. **101** — one char longer, so the comment + element break to a fresh line and collapse there.
3. **Content too long to collapse** — break before the comment, then block-style.
4. **Run of two glued comments** — `<!--a--><!--b-->` both travel with the element to the fresh line.

tsv: the opening tag never dangles after a space — the element with its glued comment prefix
moves to its own line, every line ≤100.

Prettier: keeps the opening tag on the text line and **dangles** it (`</a` / `>` split across
lines), letting the line run past printWidth — see `prettier_variant_dangle.svelte` (prettier
keeps that form stable; tsv normalizes it back to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring both formatters start from: tsv
normalizes it to `input.svelte`, prettier normalizes it to the dangle form.

## Reason

Design choice, render-free under Svelte 5. The whitespace boundary before the glued
comment-and-element run is inter-node whitespace that collapses to a single space at compile,
so turning it into a line break is render-equivalent (confirmed by `ast_diff --render`); the
glued boundary inside the run is render-significant and is never split. tsv treats a wrapping
inline element with its glued prefix uniformly — it starts a fresh line rather than leaving an
opening tag stranded at the end of the preceding text line — where prettier lets the authored
boundary decide and dangles the tag delimiters.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
