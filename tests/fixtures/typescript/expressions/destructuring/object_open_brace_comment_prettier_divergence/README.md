# object_open_brace_comment_prettier_divergence

A comment trailing an object destructuring pattern's opening `{` on the same
line (e.g. `const { // c` or `const { /* c */`) is preserved on the `{` line.
Prettier relocates it to its own line as the first property's leading comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
const { // c1                   const {
	a                                  // c1
} = o1;                            a
                                 } = o1;
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An inline block comment that hugs content in a pattern that stays
inline (`const { /* c */ a } = o`) is unchanged and matches Prettier; only the
expanding cases (a line comment after `{`, or own-line content forcing a break)
diverge.

This is the destructuring-pattern analog of the object literal `{` case
([open_brace_comment](../../objects/open_brace_comment_prettier_divergence/));
literals already preserve while patterns relocated, so extending the rule here
removes that inconsistency. Shares the `Printer::delimiter_line_comment_prefix`
helper with the literal, array, block, type-parameter `<`, and
function/constructor-type `(` cases.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
