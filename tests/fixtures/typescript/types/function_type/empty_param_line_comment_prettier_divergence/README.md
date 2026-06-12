# empty_param_line_comment_prettier_divergence

A line comment inside an **empty** function-type or constructor-type parameter
list, trailing the opening `(` on the same line (`type F = (// c`), is preserved
on the `(` line. With no parameter to lead, Prettier floats it out of the parens
to trail the now-empty `()`, forcing `=> void` onto the next line.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment out after `)`

```
// tsv                  // prettier
type F = ( // c         type F = () // c
) => void;              => void;
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `(` is a trailing comment on that
line; relocating it is a syntactic-position move. tsv preserves it in place,
which is idempotent in a single pass (Prettier's relocation is its own canonical
form).

This is the empty-params sibling of
[open_paren_comment](../open_paren_comment_prettier_divergence/) (non-empty
params, where Prettier instead drops the comment to its own line as the first
parameter's leading comment) and is consistent with the open-delimiter
trailing-comment family via the shared `Printer::delimiter_line_comment_prefix`
helper. An inline block comment (`(/* c */)`) and an own-line block comment
(`(\n/* c */\n)`) are unchanged and match Prettier; only a line comment trailing
`(` diverges. Covers function types and constructor (`new (...)`) types.

Before this, the comment swallowed the following tokens (`type F = (// c) =>
void` — invalid and non-idempotent); now it is preserved.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
