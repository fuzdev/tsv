# open_brace_comment_prettier_divergence

A comment trailing an enum body's opening `{` on the same line (e.g.
`enum E { // c` or `enum E { /* c */`) is preserved on the `{` line. Prettier
relocates it to its own line as the first member's leading comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
enum E { // c1                  enum E {
	A                                  // c1
}                                  A
                                 }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty body (`enum E { /* c */ }`) keeps the comment inline and
matches Prettier; only the expanding cases (a line comment after `{`, or
own-line content forcing a break) diverge.

This is the enum-body member of the open-delimiter family, alongside the
sibling class and interface body cases. Enum members are printed in their own
loop (`build_enum_declaration_doc`, `statements/type_declarations.rs`); it routes through
the shared `Printer::delimiter_line_comment_prefix` helper used by the
object/array literal, destructuring, block-body, `namespace`/`module`,
type-parameter `<`, and function/constructor-type `(` cases.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
