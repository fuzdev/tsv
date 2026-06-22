# open_paren_comment_prettier_divergence

A line comment trailing a function-type or constructor-type parameter list's
opening `(` on the same line (e.g. `type Fn = ( // c`) is preserved on the `(`
line. Prettier relocates it to its own line as the first parameter's leading
comment.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                       // prettier
type Fn = ( // c            type Fn = (
	p: T                        // c
) => void;                      p: T
                            ) => void;
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `(` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An inline block comment that hugs the param (`(/* c */ p)`) and an
own-line block comment (`(\n/* c */\n p)`, which both formatters keep on its own
line) are unchanged and match Prettier; only a line comment trailing `(`
diverges. Covers function types and constructor (`new (...)`) types.

Consistent with tsv's handling of the same comment position after a call's
opening `(`
([open_paren_comment](../../../expressions/calls/open_paren_comment_prettier_divergence/))
and object/array/block opening delimiters, via the shared
`Printer::delimiter_line_comment_prefix` helper.

Before this, the comment swallowed the following tokens (`type Fn = (// c p: T)
=> void` — invalid and non-idempotent); now it is preserved.

This fixture is `.ts` (not `.svelte`): the Svelte parser duplicates a
function-type `(`-leading comment in its `Root.comments` (it lists the same
span twice), so an embedded `expected.json` can't match without replicating
that Svelte bug. The formatter behavior is identical in both contexts.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
