# import_type_qualifier_line_comment_prettier_divergence

A line comment in an import type's `.`→qualifier gap
(`import('x'). // c⏎Y`). tsv keeps the comment after the `.` and drops the
qualifier to the next line:

```
type T = import('x').// c
Y;
```

**Prettier** expands the `import(…)` call and relocates the comment to trail the
source specifier (`import(⏎'x' // c⏎).Y`).

Per Comment Position Philosophy, tsv preserves the comment's authored position.
Emitting it inline (the previous behavior) let the `//` **swallow** the qualifier
— non-idempotent content loss; the line comment now forces the break (the shared
`build_trailing_comments_break_for_line`).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
