# Parser divergence: comment duplication in a populated type-literal/mapped body (call type argument)

The type argument here is a non-empty object type literal (`fn<{ // … \n a: V }>()`)
or mapped type (`fn<{ // … \n [K in keyof T]: V }>()`). A leading comment between
the `{` and the first member (for the mapped type, anywhere in the `{ [K in … ]`
header up to `in`) falls in a region acorn-typescript re-parses — its
backtrack-and-reparse fires the `onComment` callback twice, so the comment is
duplicated in the root `comments` array (and in the node's `leadingComments`).
Our parser keeps a single entry (`expected_ours.json` vs `expected_svelte.json`);
the set of distinct comments is identical — only multiplicity differs — and
`ast_diff` confirms semantic equivalence. (The call type-argument context is
incidental — the same body duplicates in any type position; this complements the
empty-body sibling `call_type_arg_empty_comment_svelte_divergence`.) The trailing
member comment (`const e`) sits outside the reparse region, so it is not
duplicated — a control that pins the leading-vs-trailing boundary.

Formatting is unaffected: the formatter finds comments by position, not by their
count in the root array, and emits each comment once at the author's placement.
See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment
Differences.
