# open_brace_comment_prettier_divergence

A comment trailing an interface body's opening `{` on the same line (e.g.
`interface I { // c` or `interface I { /* c */`) is preserved on the `{` line.
Prettier relocates it to its own line as the first member's leading comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
interface I { // c1             interface I {
	a: number;                         // c1
}                                  a: number;
                                 }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty body (`interface I { /* c */ }`) keeps the comment inline
and matches Prettier; only the expanding cases (a line comment after `{`, or
own-line content forcing a break) diverge.

This is the interface-body member of the open-delimiter family, alongside the
sibling class and enum body cases. Interface members are printed in their own
loop (`build_type_elements_doc`, `statements/type_declarations.rs`); it routes through the
shared `Printer::delimiter_line_comment_prefix` helper used by the object/array
literal, destructuring, block-body, `namespace`/`module`, type-parameter `<`,
and function/constructor-type `(` cases.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
