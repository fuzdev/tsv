# destructure_paren_comment_prettier_divergence

A `{#each … as PATTERN}` binding may wrap a binding target in grouping parens — Svelte
parses the pattern as an assignment target, where `DestructuringAssignmentTarget`
admits a parenthesized one — and the parser strips them. A comment the author wrote
**inside** those parens is preserved where it was written, like every other comment in
the pattern; prettier-plugin-svelte drops it along with the rest.

tsv: `{#each items as [b, ...(a /* c */)]}` → `{#each items as [b, ...a /* c */]}`
Prettier: `{#each items as [b, ...a]}` (comment dropped)

Covered: a rest binding in an array and an object pattern (a block comment, and a
same-line line comment whose `//` runs to end of line so the tail drops below it), a
rename value (`{ b: (a /* c */) }`), an object default value (`{ b = (1 /* c */) }`),
nested paren layers (`{ b: ((a /* c1 */) /* c2 */) }`), a shell before a following
property — with a block comment, and with a same-line line comment, below which the comma
drops (`{ b: (a // c⏎), d }` → `{ b: a // c⏎, d }`) — and an array element's default
value (`[a = (1 /* c */), b]`), and a line comment in the parens followed by another
after the `)`, at an array rest, an object rest and a property value
(`[b, ...(a // c1⏎) // c2⏎]` → `[b, ...a // c1⏎// c2⏎]`), where the pair is one trailing run
and the second comment starts the line the first one broke. Each lands where the same comment
lands without the parens — the fixed points of
[destructure_comment](../destructure_comment_prettier_divergence/)'s positions — so the
parenthesized authorings are `unformatted_ours_parens.svelte`.

The comma side follows
[destructure_comment](../destructure_comment_prettier_divergence/)'s tail-drop rule: a
binding pattern stays inline and keeps each comment on the side of the delimiter the
author wrote it, so the `//` drops the tail — comma included — to the next line; it does
not take the TypeScript list layout's `A // c⏎, B` → `A, // c` carve-out.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. prettier-plugin-svelte prints these binding patterns from a
comment-blind path and drops them. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../destructure_comment_prettier_divergence/) — the same verdict with no parens, at every pattern position
- [destructure_paren_comment](../../await/destructure_paren_comment_prettier_divergence/) — the `{#await … then}` / `{:then}` / `{:catch}` counterpart
