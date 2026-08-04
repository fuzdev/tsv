# case_block_leading_comment_run_prettier_divergence

A case label whose first consequent statement is a block hugs it (`case 1: { … }`). When a
comment run sits between the label and that block, tsv keeps each comment on the line the
author gave it — glued neighbours stay glued, own-line ones keep their own line, author blank
lines survive — and the block drops below the run, indented with the consequent. Prettier
pulls the run up onto the label line and de-indents the block to the label's level.

tsv: `case 1:` / `/* block1 */ /* block2 */` / `{` (indented with the consequent)
Prettier: `case 1: /* block1 */ /* block2 */` / `{` (at label indent)

The hug itself survives when the author left the label's rendered line to the comment —
written on the label line (`case 7: /* block */ {`), or glued past a multiline block's closing
line (`case 8`). Both trail the label, so nothing claims the line the `{` wants.

## Reason

The run is emitted by the shared leading-comment rule (prettier's own `printLeadingComment`),
which keys the separator after a comment on the source right after *that comment*, never on
where the block starts. Two consequences, both authorship-preserving: a run the author glued
(`/* block1 */ /* block2 */`) is never split, and comments the author put on separate lines
are never merged onto one.

Whether the block still hugs then falls out of that rule rather than being a second policy:
the last comment's separator is a space only when the author glued it to the `{`, and the run
itself only ever starts below the label — anything on the label's rendered line was claimed by
the label's trailing emitter, which walks a multiline block to its closing line (`case 8`) the
same way a statement's trailing run does. So any run that reaches the leading position claims
a line, and the block follows it.

That is the same answer this position already gives for a **line** comment — an own-line `//`
keeps its own line with the block indented below it, pinned by the sibling
[case_block_comment_prettier_divergence](../case_block_comment_prettier_divergence/) — so the
two comment kinds are decided by one rule instead of by their kind.

Prettier has no stable answer here: it needs four passes, and its fixed point moves the run
**across the `:`** (`case 1 /* block1 */ /* block2 */: {`), re-binding it from the block to
the case test, and pulls a label line comment inside the block body. The chain is pinned in
`audit_signature.txt`. Along the way it loses information the position carries — it reorders
(`case 3: /* block1 */ /* block2 */ // line` moves the label's own comment behind the run, and
at `default:` it emits the run backwards, `/* block2 */ /* block1 */`).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
