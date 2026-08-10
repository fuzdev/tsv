# type_assertion_comment_svelte_prettier_divergence

A comment in an `{#each}` head whose iterable carries a **TypeScript type assertion** —
`{#each items as A[] as item}`, the chained-`as` shape — is registered once and printed
once, where the author wrote it.

tsv: `{#each /* c */ items as A[] as item}` (preserved)
Prettier: `{#each items as A[] as item}` (comment dropped)

Covered positions (block comments unless noted): before the iterable, after the iterable
and before the assertion's `as`, after the assertion type and before the binding's `as`,
and a **line** comment before the iterable — which takes the head's own-line form, the
same shape a line comment produces in a head with no assertion.

The chained-`as` head is the one `{#each}` shape tsv reads with two expression sub-parses:
a partial parse stops at the first `as`, and once a second top-level `as` proves the first
was an assertion the iterable is re-parsed across the whole `items as A[]` slice. Both
sub-parses collect comments, so a comment inside the re-parsed slice was registered twice —
duplicated in the root `comments` array (a wire-AST divergence from Svelte, which registers
it once) and printed twice by whichever emitter owned the gap. The fixture pins one
registration per comment across the region; the third case, whose comment falls outside the
re-parsed slice, is the control that was never duplicated.

## Svelte divergence (parser)

Under `lang="ts"` acorn-typescript reads a bare `items as item` head as a `TSAsExpression`
too, so **every** TypeScript `{#each … as …}` head — assertion or not — goes through
canonical's type-assertion unwind, which rebuilds the expression node from the one acorn
produced and discards the `leadingComments` acorn had attached to the node it drops. tsv
keeps the comment on the surviving node. Canonical's own answer is incidental to the
authored line layout — a same-line comment loses the attachment, one on its own line keeps
it — so tsv's is the uniform one; the distinct-comment set is identical either way, and
the formatter, which locates comments by position, is unaffected. Same family as the
`remove_parens` attachment loss. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
here. prettier-plugin-svelte drops a comment leading an `{#each}` head expression whenever
the component is TypeScript, and drops one between the assertion type and the binding's
`as`; it keeps the one that falls inside the iterable's own source range
(`items /* c */ as A[]`). Prettier is also non-idempotent on the line-comment case — it
keeps the comment on the first pass and drops it on the second — so `audit_signature.txt`
pins the chain. See
[conformance_prettier_svelte.md §Svelte: each-head comments under `lang="ts"`](../../../../../../docs/conformance_prettier_svelte.md#svelte-each-head-comments-under-langts)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [type_assertion_top_level](../type_assertion_top_level/) — the comment-free chained-`as` head shapes
- [type_assertion](../type_assertion/) — an assertion nested inside a call argument, where no re-parse is needed
- [context_annotation_comment](../context_annotation_comment_svelte_prettier_divergence/) — the binding's type annotation, the other separately-parsed region of this head
