# context_annotation_comment_prettier_divergence

A comment inside a `{#each … as PATTERN: T}` binding's **type annotation** is
preserved where the author wrote it. prettier-plugin-svelte silently drops it.

tsv: `{#each items as item: /* c */ A}` (preserved)
Prettier: `{#each items as item: A}` (comment dropped)

Covered positions: an identifier binding's annotation, a destructuring binding's
annotation, and an indexed each (`as b: /* c */ C, i`), where the annotation is
followed by the `, index` tail.

The annotation region is the one part of the block-binding grammar the `{#each}`
reader parses on its own (`tsv_ts::parse_type_annotation_partial`) rather than
inside the pattern parse — it has to be, because the pattern's extent is bounded
before the annotation so `{#each xs as { a } (a.id)}` cannot read `{ a } (a.id)`
as a call. `{:then}` / `{:catch}` take their annotation inside
`parse_pattern_with_comments` instead. Both routes reach the same comment window,
which runs to the end of the **binding**, annotation included — anchoring it on
the bare pattern collapses it to the name and leaves an annotation comment
attached nowhere.

`input_invalid_annotation_trailing_comment.svelte` pins the far edge of that
region: a comment **after** the annotation type (`as item: A /* c */}`) is a
parse error in canonical Svelte, and tsv rejects it too. The sub-parse's
one-token lookahead had already skipped that trivia, so reporting its stop
position made tsv accept the head and eat the comment — a silent loss no gate
could see, since a comment that is never registered never existed as far as the
print-once ledger knows. The consumed extent is the annotation's own end
instead.

The wire is a **parser match**: canonical attaches the comment to the adjacent
node inside the annotation as `leadingComments`, and tsv reproduces it. The
`loc` columns agree too — the synthetic-`(` shift belongs to the destructure
parse alone, so an annotation comment keeps its true column.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are
syntactically valid here. prettier-plugin-svelte prints the block head from a
comment-blind path and drops them. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../destructure_comment_prettier_divergence/) — the same divergence for comments inside the destructuring pattern itself
- [destructure_comment](../../await/destructure_comment_prettier_divergence/) — the `{#await … then}` / `{:then}` / `{:catch}` face
