# call_type_arg_empty_comment_prettier_divergence

A call whose sole type argument is an empty object type literal with an interior
comment (`fn<{ /* empty */ }>()`, `fn<{ // empty }>()`).

## Formatting divergence (prettier)

tsv keeps the empty type argument hugged to the opening angle bracket. An inline
block comment stays inline in both formatters (`const a` — `fn<{ /* empty */ }>()`).
A line comment forces the body across lines: tsv keeps it hugged
(`fn<{ \n // empty \n }>()`), while prettier breaks the whole `<…>` list onto its
own indented lines (`const b`). The comment stays where the author wrote it in
both formatters — only the angle-bracket layout differs.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Single curly type-argument hug.
