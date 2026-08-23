# destructure_computed_key_jsdoc_cast_svelte_prettier_divergence

An **own-line JSDoc cast comment in an `{#each … as}` binding pattern's computed key**:

```svelte
{#each items as { [
	/** @type {A} */
	(k1)]: v1 }}
```

The pattern's computed-key `[` gap cannot hang the cast's break — no operator line to end
— so tsv **reflows** it onto the `[`'s line, the fixed point the glued authoring already
reaches (`input.svelte`, `{#each items as { [/** @type {A} */ (k1)]: v1 }}`) — the same
answer its `<script>` twin
([destructuring/computed_key_jsdoc_cast_own_line](../../../../typescript/expressions/destructuring/computed_key_jsdoc_cast_own_line_prettier_divergence/))
gives, so the pattern formats the same in both contexts.

**Formatter (vs prettier).** prettier-plugin-svelte prints these binding patterns from a
comment-blind path and **drops the cast comment and its parens outright**
(`{#each items as { [k1]: v1 }}` — `output_prettier.svelte`): content loss plus a
semantic change. tsv normalizes the `unformatted_ours_own_line.svelte` and
`unformatted_ours_break.svelte` authorings to `input.svelte` in one pass.


## Svelte divergence (parser)

The cast comment leads a **parenthesized** subexpression, and Svelte's
`parse_expression_at` sets acorn's `preserveParens: true` — so canonical attaches it to the
synthetic `ParenthesizedExpression` and then `remove_parens` discards that wrapper, taking
the `leadingComments` with it. tsv has no `ParenthesizedExpression` node (it matches
Svelte's *final* shape), so the comment attaches to the inner expression that survives.
The same stance tsv takes at every other comment-bearing island — see
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences),
whose `remove_parens` entry catalogs the family. The distinct-comment set is identical
(the comment is in the root `comments` array on both sides) and the formatter, which
locates comments by position, is unaffected.

## Reason

User comments are valuable and shouldn't be silently removed; tsv preserves the cast and
reflows the one break the gap cannot hang. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).
