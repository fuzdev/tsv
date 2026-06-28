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

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
