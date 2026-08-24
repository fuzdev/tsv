# open_brace_comment_prettier_divergence

A comment trailing an object literal's opening `{` on the same line (e.g.
`{ // c` or `{ /* c */`) is preserved on the `{` line. Prettier relocates it to
its own line as the first property's leading comment.

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. Inline block comments that hug content in an object that stays
inline (`{ /* c */ a: 1 }` on one line) are unchanged and match Prettier. The
diverging cases are the *expanding* ones: a line comment after `{`, a block
comment after `{` whose first property is on a later line (the object preserves
its authored multi-line form), or own-line content forcing a break.

Consistent with tsv's handling of the same comment position after a call's
opening `(`
([open_paren_comment](../../calls/open_paren_comment_prettier_divergence/)),
and first-element leading comments across lists.

## Author blank after the pulled comment

An author blank line between the pulled comment and the first property **survives** — the pull
moves the comment's LINE, not its membership, and a blank between a comment and what follows
it is authorship. Prettier keeps it too, from its own un-glued placement, so this half is a
plain match: `variant_blank_after_comment.svelte` (Prettier's relocated form) is dual-stable
under tsv, and the blank case in `input.svelte` is the same blank read from the authored
shape.

A blank *above* the comment is a different question and stays erased in both — it sits
against the delimiter, where every bracket drops it. That control is in `input.svelte` too.

Dropping the blank here, on the reading that a blank directly under a container's
opening line is that container's **leading-gap** blank, is the wrong reading; the derivation —
and why the answer is one value for the whole delimiter family —
is in [comments.md](../../../../../../docs/comments.md) §The delimiter-line question.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
