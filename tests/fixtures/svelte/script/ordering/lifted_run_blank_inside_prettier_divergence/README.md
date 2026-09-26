# lifted_run_blank_inside_prettier_divergence

A hoisted section whose travelling comments hold a blank line INSIDE the run: between the comment
and the `<script module>` (`<!-- c1 -->⏎⏎<script module>`), and between the two comments leading
the instance `<script>` (`<!-- c2 -->⏎⏎<!-- c3 -->⏎<script>`). That blank belongs to the run — it
travels with the comments and prints there — so between the neighbours the run was written
between it counts only as whitespace, never as a blank line. Two paragraphs with the run between
them on lines of their own (`unformatted_lifted_block_comment_blank`,
`unformatted_lifted_block_comments_blank`) keep one line break and no blank, and two text nodes
glued to the run (`unformatted_ours_lifted_text_comment_blank`,
`unformatted_ours_lifted_text_comments_blank`) join with a space — the compiler renders the
newlines inside the run as the space between `x` and `y`.

Prettier agrees on the paragraphs. Glued to the text on both sides, it deletes the whitespace
inside the run and prints `xy` (`variant_space_dropped.svelte`) — the rendered space is gone.

## Reason

◆prettier_bug. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
