# destructure_own_line_comment_prettier_divergence

A comment that forces a TypeScript destructure open forces an `{#await … then PATTERN}` /
`{:then PATTERN}` / `{:catch PATTERN}` binding pattern open the same way, and a comment the
author put on its own line keeps it. prettier-plugin-svelte drops the comment.

tsv:

```svelte
{#await promise then {
	a
	// c
}}
	<div>{a}</div>
{/await}
```

Prettier: `{#await promise then { a }}` (comment dropped).

The await binding patterns share the `{#each … as}` printer
([each/destructure_own_line_comment](../../each/destructure_own_line_comment_prettier_divergence/)
covers the full position matrix); this fixture pins the then-shorthand and the full
`{:then}` / `{:catch}` branches — an own-line `//` in an array, an own-line block comment
after a rest, a `{:catch}` pattern after a then-shorthand with its own binding
(`{#await promise then x}{x}{:catch { e⏎// c⏎}}`), the same-line authoring beside the
own-line one, and a branch nested one element deep. `unformatted_ours_inline.svelte` holds the inline authorings.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_own_line_comment](../../each/destructure_own_line_comment_prettier_divergence/) — the `{#each … as}` counterpart (full position matrix)
- [destructure_comment](../destructure_comment_prettier_divergence/) — the single-line block comments that leave the pattern inline
