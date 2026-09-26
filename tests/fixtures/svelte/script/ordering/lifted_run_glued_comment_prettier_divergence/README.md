# lifted_run_glued_comment_prettier_divergence

A `<script>` with a comment glued to it (`<!-- c --><script>…</script>`), written between two
template nodes: text, expression tags, inline elements and components glued to it on both sides,
text with a space on each side, two inline elements with a space before the run only, and two
paragraphs with a blank line on one side. The comment travels with the script to the top, and
the two neighbours join as if the run had never been there: where nothing separated them in the
source they stay glued (`x<!-- c --><script>…</script>y` renders `xy`, and so does `xy`), a space
on either side joins them with a space, and the blank line stays between the paragraphs whichever
side of the run it was written on. Each `unformatted_ours_lifted_*` variant lifts the run into one
seam of `input.svelte`; `unformatted_ours_lifted_start` writes it at the start of the document,
glued to the first text, as the control that pins where prettier's loss reaches.

Prettier joins the neighbours the same way but drops the template's **last node** — here the
`<b>y</b>` on the final line, whichever seam the run was written in — landing on
`variant_last_node_dropped.svelte` in one pass: a content loss prettier's own next pass keeps.
A comment glued to a `<script>` triggers it anywhere in the document, the start included
(`unformatted_ours_lifted_start` lands there too).

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
