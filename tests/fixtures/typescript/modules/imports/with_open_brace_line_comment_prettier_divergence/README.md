# with_open_brace_line_comment_prettier_divergence

A line comment trailing an import-attributes `with { … }` opening brace on the
same line (`with { // c`) is preserved on the `{` line. Prettier floats it past
the statement's `;`, collapsing the attributes inline.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: floats the comment to end-of-statement, past `;`

```
// tsv                              // prettier
import a from './a' with { // c     import a from './a' with { type: 'json' }; // c
	type: 'json'
};
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy) and preserves a comment trailing an opening delimiter in place
across the open-delimiter trailing-comment family, via the shared
`Printer::delimiter_line_comment_prefix` helper. The import/export **specifier**
brace (`import { // c`) already preserves; this fixture extends the same rule to
the sibling import-**attribute** `with { … }` brace, which shares the
`build_braced_hardline_comma_list` builder. Prettier's float past `;` reassociates
the comment with the whole statement rather than the attributes it introduces;
tsv keeps it on the brace line.

An inline block comment that hugs the attribute (`with { /* c */ type: 'json' }`)
and an own-line block comment are unchanged and match Prettier; only a line
comment trailing `{` diverges.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
