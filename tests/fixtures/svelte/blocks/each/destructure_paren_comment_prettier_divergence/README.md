# destructure_paren_comment_prettier_divergence

A `{#each … as PATTERN}` binding may wrap a binding target in grouping parens — Svelte
parses the pattern as an assignment target, where `DestructuringAssignmentTarget`
admits a parenthesized one — and the parser strips them. A comment the author wrote
**inside** those parens is preserved where it was written, like every other comment in
the pattern; prettier-plugin-svelte drops it along with the rest.

tsv: `{#each items as [b, ...(a /* c */)]}` → `{#each items as [b, ...a /* c */]}`
Prettier: `{#each items as [b, ...a]}` (comment dropped)

Covered: a rest binding in an array and an object pattern (a block comment, and a
same-line line comment), a rename value (`{ b: (a /* c */) }`), an object default value
(`{ b = (1 /* c */) }`), nested paren layers (`{ b: ((a /* c1 */) /* c2 */) }`), a shell
before a following property — with a block comment, and with a same-line line comment — an
array element's default value (`[a = (1 /* c */), b]`), a line comment in the parens
followed by another after the `)`, at an array rest, an object rest and a property value,
and an own-line comment in a rest's parens followed by one after the `)`. Each lands where
the same comment lands without the parens, so the parenthesized authorings are
`unformatted_ours_parens.svelte`.

A block comment on its entry's line leaves the pattern inline
(`[b, ...(a /* c */)]` → `[b, ...a /* c */]`). Every other cell breaks it open, laid out like
the same pattern in a TypeScript declaration — the rule
[destructure_own_line_comment](../destructure_own_line_comment_prettier_divergence/) pins
without parens:

- a same-line `//` trails its entry (`[b, ...(a // c⏎)]` → `[⏎b,⏎...a // c⏎]`);
- before a following property the comma moves ahead of it, the element-comma seam's
  `A // c⏎, B` → `A, // c` carve-out as in any TypeScript list
  (`{ b: (a // c⏎), d }` → `{⏎b: a, // c⏎d⏎}`);
- a second comment after the `)` starts the line the first one broke — the pair is one
  trailing run (`[b, ...(a // c1⏎) // c2⏎]` → `[⏎b,⏎...a // c1⏎// c2⏎]`);
- an own-line comment in the parens keeps its line, and the comment after the `)` follows it
  in the order the author wrote them (`[b, ...(a⏎/* i */⏎) /* t */]` →
  `[⏎b,⏎...a⏎/* i */ /* t */⏎]`, `{ b, ...(a⏎// i⏎) /* t */ }` →
  `{⏎b,⏎...a⏎// i⏎/* t */⏎}`).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. prettier-plugin-svelte prints these binding patterns from a
comment-blind path and drops them. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../destructure_comment_prettier_divergence/) — the same verdict with no parens, at every pattern position
- [destructure_paren_comment](../../await/destructure_paren_comment_prettier_divergence/) — the `{#await … then}` / `{:then}` / `{:catch}` counterpart
