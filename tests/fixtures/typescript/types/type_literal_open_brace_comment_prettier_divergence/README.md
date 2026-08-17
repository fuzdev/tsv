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

An author blank line between the pulled comment and the first member **survives** — the pull
moves the comment's LINE, not its membership, and a blank between a comment and what follows
it is authorship. Prettier keeps it too, from its own un-glued placement, so this half is a
plain match: `variant_blank_after_comment.svelte` (Prettier's relocated form) is dual-stable
under tsv, and the blank case in `input.svelte` is the same blank read from the authored
shape.

A blank *above* the comment is a different question and stays erased in both — it sits
against the delimiter, where every bracket drops it. That control is in `input.svelte` too.

tsv used to drop the blank here, on the reading that a blank directly under a container's
opening line is that container's **leading-gap** blank. The full derivation of why that
reading did not hold — and why the answer is now one value for the whole delimiter family —
is in [comments.md](../../../../../docs/comments.md) §The delimiter-line question.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
