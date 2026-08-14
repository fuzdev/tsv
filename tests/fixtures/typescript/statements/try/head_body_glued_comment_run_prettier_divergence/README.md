# head_body_glued_comment_run_prettier_divergence

A comment **run** in the `try` / `catch (e)` / bare `catch` / `finally` keyword→`{` gap,
including one the author glued onto a single line.

tsv: keeps the run where the author wrote it — glued pairs glued, separate lines separate
Prettier: absorbs the whole run into the block body, or into the catch parens

## Reason

The relocation is the divergence already sanctioned by
[line_comment_absorbed](../line_comment_absorbed_prettier_divergence/) and
[keyword_body_blank_comment](../keyword_body_blank_comment_prettier_divergence/); this
fixture adds the question neither asks — how the run's comments sit relative to **each
other**. A pair the author glued onto one line keeps that line, and a pair given separate
lines keeps those, because own-line-ness is a per-comment source question and the gap's
emitter asks it of each comment's own neighbour rather than of the body across the rest of
the run (`docs/comments.md` §Own-line-ness is a SOURCE question).

Prettier is no oracle here in either direction, and its own answer about the glue depends
entirely on where the relocation lands the run: into a `catch (e)`'s parens it becomes a
**trailing** run of the parameter and the glued pair survives; into a block body with no
statement to lead it becomes a **dangling** run, which `printDanglingComments` prints as a
`join(hardline, …)` — so prettier splits the very pair it keeps one construct over. tsv
keeps the author's line in all four gaps.

## Cases

A glued pair in each of the four gaps, a glued run ending in a `//` (which still forces
`{` onto the next line, so the comment cannot swallow it), and the separate-lines control.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
