# jsdoc_cast_own_line_svelte_prettier_divergence

An **own-line JSDoc cast comment in a directive's expression value** — the exclusion pin
for the braced-head cast-reflow family. A directive value takes the block form, which
gives the comment a properly indented line of its own, so — unlike the hugging heads,
which reflow — the authoring signal survives and tsv **keeps** it (`input.svelte`):

```svelte
<button
	on:click={
		/** @type {A} */
		(fn)
	}
>
```

Prettier **drops the cast comment and its parens outright**
(`<button on:click={fn}> text1 </button>` — `output_prettier.svelte`): comment loss plus
a semantic change. tsv preserves and holds its stable block form.

The other expression-value directives (`bind:` / `class:` / `use:` / `transition:` /
`animate:`) share this arm; `style:` does not — Svelte models its value as an expression
tag, and it reflows with the tag family
([attributes/jsdoc_cast_own_line](../../../attributes/jsdoc_cast_own_line_svelte_prettier_divergence/)).

**Parser (vs Svelte).** Svelte parses these expressions with `preserveParens: true`,
then `remove_parens` discards the wrapper **and its `leadingComments`**, so the cast
comment survives only in the root `comments` array; tsv (no `ParenthesizedExpression`
node) attaches it to the inner expression (`expected_ours.json` vs
`expected_svelte.json`). The comment is never lost; only its attachment differs. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Reason

Own-line-ness is authoring signal wherever the container keeps lines, and the directive's
block form does; prettier's deletion is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Own-line JSDoc cast at a braced head](../../../../../../docs/conformance_prettier_svelte.md#svelte-own-line-jsdoc-cast-at-a-braced-head);
the cast-preservation frame is
[conformance_prettier_ts_comments.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
