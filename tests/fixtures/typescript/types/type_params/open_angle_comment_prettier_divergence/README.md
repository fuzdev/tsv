# open_angle_comment_prettier_divergence

A line comment trailing a type-parameter list's opening `<` on the same line
(e.g. `function f< // c`) is preserved on the `<` line. Prettier relocates it to
its own line as the first type parameter's leading comment.

tsv: keeps the comment trailing `<` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                       // prettier
function f< // c             function f<
	T                           // c
>(p: T) {}                      T
                            >(p: T) {}
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `<` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An inline block comment that hugs content (`<T /* c */>`, or
`</* c */ T>` when it fits) and an own-line block comment (`<\n/* c */\n T>`,
which both formatters keep on its own line) are unchanged and match Prettier;
only a line comment trailing `<` diverges. Covers function declarations,
classes, interfaces, type aliases, and arrow functions (the shared
type-parameter-declaration printer).

Consistent with tsv's handling of the same comment position after a call's
opening `(`
([open_paren_comment](../../../expressions/calls/open_paren_comment_prettier_divergence/))
and object/array/block opening delimiters
([open_brace_comment](../../../expressions/objects/open_brace_comment_prettier_divergence/)),
via the shared `Printer::delimiter_line_comment_prefix` helper.

Before this, the comment was dropped entirely (a content-loss bug).

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
