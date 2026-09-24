# destructure_paren_comment_prettier_divergence

A comment inside the grouping parens the parser stripped from an `{#await … then PATTERN}`
/ `{:then PATTERN}` / `{:catch PATTERN}` binding target is preserved where the author
wrote it. prettier-plugin-svelte drops it.

tsv: `{#await promise then [b, ...(a /* c */)]}` → `{#await promise then [b, ...a /* c */]}`
Prettier: `{#await promise then [b, ...a]}` (comment dropped)

The await binding patterns share the comment-aware printer of `{#each … as}`
([each/destructure_paren_comment](../../each/destructure_paren_comment_prettier_divergence/)
covers the full position matrix); this fixture pins the then-shorthand and the full
`{:then}` / `{:catch}` branches, which reach it by the other parse route. The
parenthesized authorings are `unformatted_ours_parens.svelte`.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../destructure_comment_prettier_divergence/) — the same verdict with no parens
