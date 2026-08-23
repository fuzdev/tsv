# ts_head_comment_duplication_svelte_prettier_divergence

Under `lang="ts"`, canonical Svelte lists some `{#each}`-head comments **twice** in
the root `comments` array — and attaches both copies. tsv lists each once.

## Svelte divergence (parser)

acorn-typescript reads even a bare `items as item` head as a `TSAsExpression`, so
**every** TS each head takes canonical's type-assertion unwind: `read_expression`
parses past the `as`, its `onComment` pushes everything it scanned into the shared
`parser.root.comments`, and the `{#each}` reader then discards that node and rewinds
`parser.index` to the `as` so the binding / index / key can be read properly — whose
own parses push the same comments again. `add_comments` re-filters the *whole
accumulated* array rather than its own parse's pushes, so both copies attach as well.
The discarded parse is what doubles them, which is why the trigger is how far that
parse reached, not where the comment sits:

- **Inside the binding pattern** — always. The pattern is the `as` type the
  speculative parse read (`as { /* c */ b }`).
- **Inside the key parens** — only with a `, index` ahead of it. The comma makes the
  speculative parse a sequence expression that runs on through the key
  (`as c, i (/* c */ c)`); with no index it stops before the parens, and the same
  comment is listed once (`as d (/* c */ d)` — the third case, a null control).

The fourth case is the other null control: an `{#if}` head takes no unwind at all.
Together they isolate the discarded parse's reach as the cause rather than "a
comment in a TS block head".

tsv answers the `as` question directly (`tsv_ts::TopLevelAs`) instead of speculating,
so there is no discarded parse and each comment exists once, attached once. Declining
to reproduce it is the same stance as every other entry in
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences)
— a duplicate is an artifact of a parse tsv does not perform. The distinct-comment
set is identical and the formatter, which locates comments by position, is unaffected.

## Prettier divergence (formatter)

Only the second case: prettier-plugin-svelte prints binding patterns from a
comment-blind path and drops the interior comment — the same drop
[destructure_comment](../destructure_comment_prettier_divergence/) owns, seen here in a
TS component. It keeps all three others. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [type_assertion_comment](../type_assertion_comment_svelte_prettier_divergence/) — the same unwind, seen from its *other* consequence: the attachment it DROPS from the node it discards
- [no_as_key_comment](../no_as_key_comment_svelte_prettier_divergence/) — the no-`as` head, which doubles a key comment by the same sequence-expression mechanism without needing `lang="ts"`
