# destructure_comment_prettier_divergence

A comment placed **inside** an `{#await … then PATTERN}` / `{:then PATTERN}` /
`{:catch PATTERN}` destructuring binding pattern is preserved where the author wrote it.
prettier-plugin-svelte silently drops it.

tsv: `{#await promise then { a = /* c */ 1 }}` (preserved)
Prettier: `{#await promise then { a = 1 }}` (comment dropped)

The await binding patterns share the same comment-aware printer as `{#each … as}`
([each/destructure_comment](../../each/destructure_comment_prettier_divergence/)
covers the full position matrix); this fixture pins the then-shorthand and the full
`{:then}` / `{:catch}` branches. (Earlier, a comment in the *then-shorthand* pattern was
also mis-relocated out to trail the awaited expression — `{#await promise /* c */ then …}`
— because the expression's trailing-comment range spanned the whole head; the range now
stops at the pattern, so the comment stays inside it.)

The wire is a **parser match** here as it is for `{#each}`: canonical attaches each
interior comment to its adjacent pattern node, and tsv reproduces the attachment from the
same window. These branches reach it by the other route — `{:then}` / `{:catch}` take
their binding (and any `: T` annotation) in **one** sub-parse, where `{#each}` splits the
pattern and the annotation into two.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../../each/destructure_comment_prettier_divergence/) — the `{#each … as}` counterpart (full position matrix)
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same drop-vs-preserve family for trailing comments in template expressions
