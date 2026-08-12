# debug_jsdoc_cast_svelte_prettier_divergence

**JSDoc casts in `{@debug}` arguments** — around a single identifier
(`{@debug /** @type {A} */ (a)}`), around the whole comma list
(`{@debug /** @type {B} */ (b, c)}`), and on one element of the list
(`{@debug d, /** @type {C} */ (e)}`). Svelte's identifiers-only rule runs after
`remove_parens`, which sees nothing but parens plus a comment here, so every form is a
valid debug tag; tsv validates the rule through the cast wrapper the same way while
keeping the cast in place.

tsv: casts preserved as authored.
Prettier: `{@debug a}` / `{@debug b, c}` / `{@debug d, e}` — it strips the cast comment
**and its parens** (comment loss plus a semantic change), the same strip as its plain
`{@debug}` comment drop.

**Parser (vs Svelte).** Svelte parses these expressions with `preserveParens: true`,
then `remove_parens` discards the wrapper **and its `leadingComments`**, so the cast
comment survives only in the root `comments` array; tsv (no `ParenthesizedExpression`
node) attaches it to the inner expression (`expected_ours.json` vs
`expected_svelte.json`). The comment is never lost; only its attachment differs. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Reason

Content preservation — comments in debug statements carry developer intent, and a cast's
parens carry its binding; stripping either is silent content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../../docs/conformance_prettier_svelte.md#svelte-elements)
(the `@debug comments` catalog entry); the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).

## Related

- [debug_comment](../debug_comment_prettier_divergence/) — the plain-comment strip
- [render_jsdoc_cast_root](../../render_jsdoc_cast_root_svelte_prettier_divergence/) —
  the same root cast at `{@render}`
