# type_argument_open_angle_comment_prettier_divergence

A comment trailing a multi-argument type-argument list's opening `<` on the
same line (e.g. `Map< // c` or `Map< /* c */`) is preserved on the `<` line.
Prettier relocates it to its own line as the first argument's leading comment.

This is the type-_argument_ list (`Map<A, B>` — the args passed to a generic),
distinct from the type-_parameter_ _declaration_ `<` (`function f<T>`), which is
covered separately by
[type_params/open_angle_comment](../type_params/open_angle_comment_prettier_divergence/).

tsv: keeps the comment trailing `<` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
type A = Map< // c1             type A = Map<
	string,                            // c1
	number                           string,
>;                                 number
                                 >;
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `<` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable.

**Multi-argument only.** A single-argument list with a leading line comment
(`Array< // c\n a>`) hugs `<`/`>` in both formatters and already matches Prettier
— it is not a divergence and is left unchanged. Empty argument lists are not
representable. Only the expanding multi-argument cases (a line comment after
`<`, or own-line content forcing a break) diverge.

This is the type-argument member of the open-delimiter family. Multi-argument
type-argument lists are printed in their own multiline path
(`build_type_arguments_doc_with_line_comments`, `statements/type_declarations.rs`); it routes
through the shared `Printer::delimiter_line_comment_prefix` helper (with
`build_leading_comments_multiline_after_delim` to drop the pulled comment from
the first argument) used by the object/array literal, destructuring, block-body,
`namespace`/`module`, class/interface/enum body, type literal, import/export
specifier, and tuple-type cases. Type arguments in call/`new` _expression_
position (`foo<A, B>()`) use a separate builder and still relocate.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
