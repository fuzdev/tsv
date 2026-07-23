# inline_break_before_comment_glued_word_long_prettier_divergence

The glued-prefix companion of `inline_break_before_comment_glued_long`. Here a glued **text
word** *also* precedes the comment, so `word<!--c--><a …>` is one indivisible glued run: the
break lands at the last **whitespace** boundary *before the word*, and the whole run — word,
comment, and element — moves to the fresh line together. This mirrors the glued text prefix in
`inline_break_before_glued_long`, with a comment as an additional glued node between the word
and the element. Breaking anywhere inside the run (before the comment, or between the comment
and the element) is render-significant — it would inject a rendered space — so tsv only ever
breaks at the whitespace boundary before the run.

Cases (in order):

1. **Fits at exactly 100** — the run stays inline on the text line, no break (control).
2. **101** — one char longer, so the run breaks before the glued word and collapses on its fresh line.

tsv: the run (`glued<!--c--><a …>content</a>.`) moves to its own line together, every line ≤100.

Prettier: pulls the glued word back onto the text line and **dangles** the element (`</a` / `>`
split across lines), letting the line run past printWidth — see `output_prettier.svelte`
(prettier's stable form; tsv normalizes it back to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring both formatters start from: tsv
normalizes it to `input.svelte`, prettier normalizes it to `output_prettier.svelte`.

## Reason

Design choice, render-free under Svelte 5 for the *whitespace* boundary before the run;
render-significant for every *glued* boundary inside it, which is therefore never split. The
whitespace boundary collapses to a single space at compile, so turning it into a line break is
render-equivalent (confirmed by `ast_diff --render`).
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
