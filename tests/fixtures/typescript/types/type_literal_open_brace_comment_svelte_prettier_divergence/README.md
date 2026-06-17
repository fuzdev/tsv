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
unchanged and match Prettier (see the non-divergent sibling
[type_literal_open_brace_comment](../type_literal_open_brace_comment/)); only
the expanding line-comment cases (a line comment after `{`, or own-line content
forcing a break) diverge.

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

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.

## Parser — svelte divergence

The same in-construct comment is also duplicated in acorn-typescript's root
`comments` array (its backtrack-and-reparse fires `onComment` twice); our parser
keeps a single entry (`expected_ours.json` vs `expected_svelte.json`). The set of
distinct comments is identical — only multiplicity differs — and `ast_diff`
confirms semantic equivalence. See
[conformance_svelte.md](../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
