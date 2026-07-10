# method_open_paren_line_comment_prettier_divergence

A line comment trailing a class **method**, **constructor**, or **setter**
parameter-list opening `(` on the same line (`method( // c`,
`constructor( // c`, `set value( // c`) is preserved on the `(` line. Prettier
relocates it to its own line as the first parameter's leading comment.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                    // prettier
class C {                 class C {
	method( // c              method(
		a: number                 // c
	) {}                          a: number
}                             ) {}
                          }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy) and preserves a comment trailing an opening delimiter in place
across the open-delimiter trailing-comment family (call `(`, function-type `(`,
call/construct signature `(`, object/array `{`/`[`, type-param `<`, …), via the
shared `Printer::delimiter_line_comment_prefix` helper. Class methods,
constructors, and setters print their parameter list through the same value-level
parameter printer as function declarations/expressions/arrows
([open_paren_line_comment](../../../declarations/function/open_paren_line_comment_prettier_divergence/)),
so they preserve the `(`-line comment uniformly.

An inline block comment that hugs the param (`method( /* c */ a)`) and an
own-line block comment are unchanged and match Prettier; only a line comment
trailing `(` diverges.

Before this, the comment ran to end-of-line and swallowed the following tokens
(`method(// c a: number)` — invalid and non-idempotent); now it is preserved.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
