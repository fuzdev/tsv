# in_of_header_comment_run_prettier_divergence

A **run** of comments in one gap of a broken `for-in`/`for-of` header — the
`(`→binding gap, the binding→keyword gap, the keyword→iterable gap, and the
iterable→`)` gap all answer the same way.

tsv keeps the run in place and in order, each comment on the line the author
gave it: a comment glued to the previous one keeps that line, an own-line one
keeps its own, and a blank line between two of them is preserved. A `//` in the
run therefore always ends its line.

Prettier collapses the header inline and relocates the run out of it, which
**reorders** the comments (`for (a of /* c2 */ // c1`) and, where the run ends
in a `//` followed by a multiline block, folds the line comment *into* the block
(`/* c2 // c1 ⏎ c3 */`) — the line comment stops being a comment of its own. Its
form here also needs three passes to settle (pinned by `audit_signature.txt`).

The glued run (`b /* c1 */ /* c2 */ // c3`) is the control: both formatters keep
that line intact, and only its position differs.

The `(`→binding gap is a **leading** run rather than a trailing one, so it splits
at the binding: the glued suffix leads it inline (`/* c1 */ /* c2 */ h`) and the
rest take their own lines, with an author blank between two of them preserved.
Prettier agrees on both inside its own collapsed header — only the header layout
differs there.

## Reason

A run's separator is a question about each comment's own neighbours, and a `//`
runs to end of line — so a second comment emitted after one on the same line
stops being a comment and becomes that comment's text. Preserving the author's
line structure is what keeps the run lossless, and is the same rule the C-style
`for` header's clause gaps already follow (`clause_leading_comment_run`,
`clause_leading_glued_run`, `clause_own_line_comment`).

Single-comment gaps in the same header are pinned by
`in_of_own_line_comment_prettier_divergence` and
`of_line_comment_prettier_divergence`; this fixture is the multi-comment case.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
