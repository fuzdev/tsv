# open_paren_line_comment_prettier_divergence

A line comment trailing a **value-level function definition**'s parameter-list
opening `(` on the same line (`function fn( // c`, `function ( // c`, `( // c
) => …`) is preserved on the `(` line. Prettier moves it off that line.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: relocates it (function expression / arrow: onto its own line as the
first parameter's leading comment; function **declaration**: floated past the
whole declaration, collapsing the params inline)

```
// tsv                       // prettier
function fn( // c            function fn(a: number) {} // c
	a: number
) {}

const arrow = ( // c         const arrow = (
	a: number                    // c
) => a;                          a: number
                             ) => a;
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy) and preserves a comment trailing an opening delimiter in place
across the open-delimiter trailing-comment family (call `(`, function-type `(`,
call/construct signature `(`, object/array `{`/`[`, type-param `<`, …), via the
shared `Printer::delimiter_line_comment_prefix` helper. This fixture extends that
uniform rule to value-level function definitions — function declarations,
function expressions, and arrows — which previously relocated the comment (an
un-principled split from the function-*type* and *signature* `(`, which already
preserved).

Prettier is inconsistent here: it floats the comment past a function
declaration but relocates it onto its own line for a function expression / arrow.
Either way it moves the comment off the `(` line; tsv keeps it in place.

An inline block comment that hugs the param (`( /* c */ a)`) and an own-line
block comment (`(\n/* c */\n a)`, which both formatters keep on its own line)
are unchanged and match Prettier; only a line comment trailing `(` diverges.

Before this, the comment ran to end-of-line and swallowed the following tokens
(`function fn(// c a: number)` — invalid and non-idempotent); now it is
preserved.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
