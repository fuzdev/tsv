# call_type_arg_member_comment_svelte_prettier_divergence

A call whose sole type argument is an object type literal or a mapped type, with
a leading comment in the body (`fn<{ // … \n a: V }>()`,
`fn<{ /* … */ \n [K in keyof T]: V }>()`). Two independent divergences meet here.

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

## Parser divergence (svelte/acorn)

A leading comment between the `{` and the first member (for the mapped type,
anywhere in the `{ [K in … ]` header up to `in`) falls in a region
acorn-typescript re-parses — its backtrack-and-reparse fires the `onComment`
callback twice, so the comment is duplicated in the root `comments` array (and in
the node's `leadingComments`). tsv keeps a single entry (`expected_ours.json` vs
`expected_svelte.json`); the set of distinct comments is identical — only
multiplicity differs — and `ast_diff` confirms semantic equivalence. The trailing
member comment (`const e`) sits outside the reparse region, so it is not
duplicated — a control that pins the leading-vs-trailing boundary.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
