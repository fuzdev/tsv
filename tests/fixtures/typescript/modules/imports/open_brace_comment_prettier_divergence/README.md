# open_brace_comment_prettier_divergence

A comment trailing an import specifier list's opening `{` on the same line
(e.g. `import { // c` or `import { /* c */`) is preserved on the `{` line.
Prettier relocates it to its own line as the first specifier's leading comment.

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
import { // c1                  import {
	a                                  // c1
} from './a';                      a
                                 } from './a';
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty specifier list (`import {} from 'x'`) has no first
specifier and is unaffected; only the expanding cases (a line comment after
`{`, or own-line content forcing a break) diverge.

This is the import-specifier member of the open-delimiter family, alongside the
sibling export-specifier case. Import/export specifier lists share the multiline
comma-list builder (`build_hardline_comma_list`, `statements/modules.rs`); it
routes through the shared `Printer::delimiter_line_comment_prefix` helper used
by the object/array literal, destructuring, block-body, `namespace`/`module`,
class/interface/enum body, and type literal cases. The `with {…}` import-
attribute brace is a separate, rarer delimiter and keeps relocating for now.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
