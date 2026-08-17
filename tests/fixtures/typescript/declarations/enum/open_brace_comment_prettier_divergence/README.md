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
is in [comments.md](../../../../../../docs/comments.md) §The delimiter-line question.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
