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

An author blank line between the pulled comment and the first property does not
survive — tsv prints the property directly under the `{` line. That is a
*consequence* of the position above, not a second choice: both formatters
discard a blank in the **leading gap** (an opening delimiter's line → the first
item) and both preserve one *between* items. Keeping the comment on the `{`
line leaves the blank in the leading gap, so tsv's own leading-gap rule drops
it; Prettier makes the comment the first body item, which moves the blank into
an inter-item gap it then keeps. The derivation runs both ways — hand tsv
Prettier's relocated form and tsv preserves the blank itself, which is why
`variant_blank_after_comment.svelte` is dual-stable while
`unformatted_ours_blank_after_comment.svelte` (the authored shape) normalizes
back to `input.svelte`.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
