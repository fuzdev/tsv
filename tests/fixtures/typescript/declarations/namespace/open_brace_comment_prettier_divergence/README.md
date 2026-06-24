# open_brace_comment_prettier_divergence

A comment trailing a `namespace`/`module` body's opening `{` on the same line
(e.g. `namespace N { // c` or `module M { /* c */`) is preserved on the `{`
line. Prettier relocates it to its own line as the first statement's leading
comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
namespace N { // c1             namespace N {
	const a = 1;                       // c1
}                                  const a = 1;
                                 }
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty body (`namespace N { /* c */ }`) keeps the comment inline
and matches Prettier; only the expanding cases (a line comment after `{`, or
own-line content forcing a break) diverge.

This is the `namespace`/`module`-body analog of the block-body `{` case
([block_open_brace_comment](../../../statements/block_open_brace_comment_prettier_divergence/)).
Namespace bodies share the per-statement walk (`build_statement_list_docs`)
with block statements, so the same `delimiter_pull_pos` threading applies; both
route through the shared `Printer::delimiter_line_comment_prefix` helper used by
the object/array literal, destructuring, type-parameter `<`, and
function/constructor-type `(` cases.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
