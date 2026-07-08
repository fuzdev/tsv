# call_type_arg_member_comment_prettier_divergence

A call whose sole type argument is an object type literal or a mapped type, with
a leading comment in the body (`fn<{ // … \n a: V }>()`,
`fn<{ /* … */ \n [K in keyof T]: V }>()`).

## Formatting divergence (prettier)

tsv keeps the type argument hugged to the opening angle bracket — `fn<{ … }>()` —
for every curly type argument, breaking only the body block-style when a comment
forces it across lines. Prettier instead breaks the whole `<…>` list onto its own
indented lines when the body is a comment-bearing mapped type (`const a`,
`const b`), while keeping a populated object literal hugged (`const c`, `const d`).
The trailing member comment (`const e`) hugs in both. The comment stays where the
author wrote it in both formatters — only the angle-bracket layout differs.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Single curly type-argument hug.
