# head_body_glued_comment_run_prettier_divergence

A comment **run** in the `switch (…)`→`{` gap, including one the author glued onto a
single line.

tsv: keeps the run where the author wrote it — glued pairs glued, separate lines separate
Prettier: absorbs the whole run into the switch body, leading the first `case`

## Reason

The relocation is the divergence already sanctioned by
[head_body_line_comment](../head_body_line_comment_prettier_divergence/); this fixture
adds the question that one does not ask — how the run's comments sit relative to **each
other**. A pair the author glued onto one line keeps that line, and a pair given separate
lines keeps those, because own-line-ness is a per-comment source question and the gap's
emitter asks it of each comment's own neighbour rather than of the body across the rest of
the run (`docs/comments.md` §Own-line-ness is a SOURCE question). Reading it off the
gap's line buckets instead split every glued pair.

Prettier is no oracle for the gap itself — it moves the run into the body — but it agrees
about the glue once it gets there: relocated in front of `case 1:` the comments become
that statement's **leading** run, and `printLeadingComment` keeps the glued pair on one
line, exactly as tsv does in place.

## Cases

A glued pair, a glued run ending in a `//` (which still forces `{` onto the next line, so
the comment cannot swallow it), and the separate-lines control.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
