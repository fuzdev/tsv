# destructure_comment_svelte_prettier_divergence

A comment placed **inside** an `{#await … then PATTERN}` / `{:then PATTERN}` /
`{:catch PATTERN}` destructuring binding pattern is preserved where the author wrote it.
prettier-plugin-svelte silently drops it.

tsv: `{#await promise then { a = /* c */ 1 }}` (preserved)
Prettier: `{#await promise then { a = 1 }}` (comment dropped)

The await binding patterns share the same comment-aware printer as `{#each … as}`
([each/destructure_comment](../../each/destructure_comment_svelte_prettier_divergence/)
covers the full position matrix); this fixture pins the then-shorthand and the full
`{:then}` / `{:catch}` branches. (Earlier, a comment in the *then-shorthand* pattern was
also mis-relocated out to trail the awaited expression — `{#await promise /* c */ then …}`
— because the expression's trailing-comment range spanned the whole head; the range now
stops at the pattern, so the comment stays inside it.)

## Svelte divergence (parser)

acorn parses the binding pattern and attaches the comment to the adjacent node as
`leadingComments` / `trailingComments`; tsv keeps each comment once in the root `comments`
array (detached model), so `expected_ours.json` omits the attachments
`expected_svelte.json` carries. Distinct-comment set identical, `ast_diff` equivalent,
formatter unaffected. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. See
[conformance_prettier.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../../each/destructure_comment_svelte_prettier_divergence/) — the `{#each … as}` counterpart (full position matrix)
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same drop-vs-preserve family for trailing comments in template expressions
