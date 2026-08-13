# block_open_brace_comment_prettier_divergence

A comment trailing a block body's opening `{` on the same line (e.g.
`function f() { // c` or `=> { /* c */`) is preserved on the `{` line. Prettier
relocates it to its own line as the body's leading comment. Applies to function
bodies, plain blocks, and arrow block bodies.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
function f() { // c             function f() {
	body();                            // c
}                                  body();
                                 }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An inline block comment in a body that stays inline (e.g. an empty
body `{ /* c */ }`) is unchanged and matches Prettier; only the cases where the
body breaks across lines diverge.

Consistent with tsv's handling of the same comment position after a call's
opening `(`
([open_paren_comment](../../expressions/calls/open_paren_comment_prettier_divergence/)),
an object literal's opening `{`
([open_brace_comment](../../expressions/objects/open_brace_comment_prettier_divergence/)),
and an array literal's opening `[`
([open_bracket_comment](../../expressions/arrays/open_bracket_comment_prettier_divergence/)).

## Author blank after the pulled comment

An author blank line between the pulled comment and the first statement does not
survive — tsv prints the statement directly under the `{` line. That is a
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

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
