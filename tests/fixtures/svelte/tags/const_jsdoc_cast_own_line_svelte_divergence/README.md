# const_jsdoc_cast_own_line_svelte_divergence

An **own-line JSDoc cast comment at a `{@const}` initializer** — the hang pin for the
braced-head cast-reflow family. `{@const}` is the one braced head with a real operator
line to end: the `=` hangs the value exactly like a TS declarator, so the own-line
authoring keeps its line (`input.svelte`):

```svelte
{#if aa}
	{@const bb =
		/** @type {A} */
		(cc)}
	text1
{/if}
```

**Formatter: both tools agree** — prettier produces the identical hang, so there is no
formatter divergence to pin (no `output_prettier.svelte`), and this shape must stay
untouched by the hugging heads' reflow.

**Parser (vs Svelte).** Svelte parses the init with `preserveParens: true`, then
`remove_parens` discards the wrapper **and its `leadingComments`**, so the cast comment
survives only in the root `comments` array. tsv (no `ParenthesizedExpression` node)
attaches it to the inner expression (`expected_ours.json` vs `expected_svelte.json`).
The comment is never lost; only its attachment differs. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../docs/conformance_svelte.md#comment-attachment-differences).
