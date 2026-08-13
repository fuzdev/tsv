# open_brace_comment_prettier_divergence

A comment trailing a class body's opening `{` on the same line (e.g.
`class C { // c` or `class C { /* c */`) is preserved on the `{` line.
Prettier relocates it to its own line as the first member's leading comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
class C { // c1                 class C {
	a = 1;                             // c1
}                                  a = 1;
                                 }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty body (`class C { /* c */ }`) keeps the comment inline and
matches Prettier; only the expanding cases (a line comment after `{`, or
own-line content forcing a break) diverge.

This is the class-body member of the open-delimiter family, alongside the
sibling interface and enum body cases. Class members are printed in their own
loop (`build_class_body_doc`, `statements/class.rs`); it routes through the
shared `Printer::delimiter_line_comment_prefix` helper used by the object/array
literal, destructuring, block-body, `namespace`/`module`, type-parameter `<`,
and function/constructor-type `(` cases.

## Author blank after the pulled comment

An author blank line between the pulled comment and the first member does not
survive — tsv prints the member directly under the `{` line. That is a
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

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
