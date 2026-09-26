# destructure_own_line_comment_prettier_divergence

A comment that forces a TypeScript destructure open — a `//` line comment, a block comment
on its own line, a multi-line block comment — forces a `{#each … as PATTERN}` binding
pattern open the same way: the pattern lays out like its TypeScript twin, and a comment the
author put on its own line keeps it. prettier-plugin-svelte drops the comment.

tsv:

```svelte
{#each items as {
	a
	// c
}}
	<div>{a}</div>
{/each}
```

Prettier: `{#each items as { a }}` (comment dropped).

The layout is the TypeScript twin's (`const { a⏎// c⏎} = x`) read into the head: one entry
per line one level in, the closing brace back at the head's column with whatever follows it (`}}`, `}, i}`,
`}, i (a)}`), and the body expanded below, since a broken head expands its block.
It is the layout `{@const}` and `{#snippet}` parameters already take — both reach the
TypeScript printer, and prettier prints it for them too.

Covered: an own-line `//` after the last property, after a rest and after a default; before
the first property; between two properties; a `//` glued to the opening `{` or `[`, which
keeps that line (`{ // c⏎a }` → `{ // c⏎→a⏎}`, the delimiter-line rule); an array's last
element, rest, a comment between two elements, and one after a hole (`[, a⏎// c⏎]` →
`[⏎→,⏎→a⏎→// c⏎]`); an empty pattern holding only a `//` (`{⏎→// c⏎}`, `[⏎→// c⏎]`, the twin's dangling run); a nested pattern, whose break breaks the outer one; a run of two
comments; an author blank above the comment and one between two entries (both kept, as the
twin keeps them); own-line block comments before, after, between two properties and in an
array; an own-line block comment inside an object or array default value, which breaks the value and so the pattern, as it breaks the twin's — and at a default object's key gap, where the comment collapses onto the key's line and the object it opened stays open, as the twin's does (`{ a = { b:⏎/* c */1 } }`) — (a call or arrow default is [destructure_default_value_comment](../destructure_default_value_comment_prettier_divergence/)'s, since prettier-plugin-svelte throws on one); a multi-line block comment written on the entry's line; the same-line authoring
beside the own-line one; the index, index-and-key, and key-only tails; and the pattern
nested one element deep and one block deep.

Two variants hold the other authorings. `unformatted_ours_inline.svelte` writes each pattern
inline, as the author would — among them a `//` before a comma, which moves past it
(`{ a // c⏎, b }` → `a, // c⏎b`, the element-comma seam's carve-out, as in any TypeScript
list). `unformatted_ours_hugged.svelte` also hugs every body and every enclosing element and
block (`{#if ok}{#each items as { a⏎// c⏎}}<div>{a}</div>{/each}{/if}`): a broken head
expands its block wherever it sits.
Prettier answers that authoring with its comments dropped and every hug kept —
`divergent_variant_comment_dropped.svelte`, a form tsv keeps too, since with no comment left
nothing breaks a head, save the two emptied patterns: prettier leaves them `{ }`, which tsv
tightens to `{}`, as it prints every empty pattern
([destructure_empty](../destructure_empty_prettier_divergence/)).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. prettier-plugin-svelte prints these binding patterns from a
comment-blind path and drops them, so it has no layout of its own to follow here. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments),
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
for the principle.

## Related

- [destructure_own_line_comment](../../await/destructure_own_line_comment_prettier_divergence/) — the `{#await … then}` / `{:then}` / `{:catch}` counterpart
- [destructure_comment](../destructure_comment_prettier_divergence/) — the single-line block comments that leave the pattern inline
- [destructure_paren_comment](../destructure_paren_comment_prettier_divergence/) — the same layout for a comment inside a binding's stripped grouping parens
