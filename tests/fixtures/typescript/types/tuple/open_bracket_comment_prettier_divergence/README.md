# open_bracket_comment_prettier_divergence

A comment trailing a tuple type's opening `[` on the same line (e.g.
`type T = [ // c` or `type T = [ /* c */`) is preserved on the `[` line.
Prettier relocates it to its own line as the first element's leading comment.

tsv: keeps the comment trailing `[` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
type A = [ // c1                type A = [
	string                             // c1
];                                 string
                                 ];
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `[` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty tuple (`type T = []`) has no first element and is
unaffected; tuple types have no elision (no holes), so the first element is
always present. Only the expanding cases (a line comment after `[`, or own-line
content forcing a break) diverge.

This is the tuple-type member of the open-delimiter family, alongside the array
literal and array destructuring `[` cases. Tuple elements are printed in their
own multiline path (`build_tuple_type_doc_with_line_comments`,
`types/composite.rs`); it routes through the shared
`Printer::delimiter_line_comment_prefix` helper (with
`build_leading_comments_multiline_after_delim` to drop the pulled comment from
the first element) used by the object/array literal, destructuring, block-body,
`namespace`/`module`, class/interface/enum body, type literal, and import/export
specifier cases.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
