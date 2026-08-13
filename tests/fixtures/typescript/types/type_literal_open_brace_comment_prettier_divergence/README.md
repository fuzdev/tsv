# type_literal_open_brace_comment_prettier_divergence

A comment trailing a type literal's opening `{` on the same line (e.g.
`type A = { // c` or `type A = { /* c */`) is preserved on the `{` line.
Prettier relocates it to its own line as the first member's leading comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
type A = { // c1                type A = {
	a: number;                         // c1
};                                 a: number;
                                 };
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An inline block comment that hugs content in a literal that stays
inline (`type B = {/* c */ a: number}`) and an own-line block comment are
unchanged and match Prettier (those formatter-non-divergent block cases live in
the parser-only sibling
[type_literal_open_brace_comment](../type_literal_open_brace_comment/));
only the expanding line-comment cases (a line comment after `{`, or own-line
content forcing a break) diverge.

This is the type-literal member of the open-delimiter family. Type-literal
members are printed in their own multiline path (`build_type_literal_doc_inner`
→ `build_multiline_member_prefix_doc`, `types/type_literal.rs`); it routes
through the shared `Printer::delimiter_line_comment_prefix` helper used by the
object/array literal, destructuring, block-body, `namespace`/`module`, and
class/interface/enum body cases. This covers the standard type-literal contexts
(type aliases, annotations, function-param literals, intersection-trailing
objects); the specialized union-member / parenthesized-intersection *alignment*
rendering (`type T = | { // c } | B`) keeps relocating — a rarer sub-case that
uses a different builder.

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

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
