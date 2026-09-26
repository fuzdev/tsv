# destructure_default_value_comment_prettier_divergence

An own-line block comment inside a `{#each … as PATTERN}` binding's **default value** — a
call's arguments, an arrow function's body — breaks that value the way it breaks it in the
TypeScript twin (`const { a = f(1⏎/* c */⏎) } = x`), and a value that breaks breaks the pattern
holding it, so the pattern lays out like the twin: one entry per line, the block's body
expanded below.

tsv:

```svelte
{#each items as {
	a = f(
		1
		/* c */
	)
}}
	<div>{a}</div>
{/each}
```

`unformatted_ours_inline.svelte` holds the inline authorings, which reach the same form.

## Prettier divergence (formatter)

No oracle: prettier-plugin-svelte's block-pattern printer has no arm for a call or an arrow
function as a default value and throws (`unknown node type: CallExpression`), which
`prettier_rejects.txt` pins. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_own_line_comment](../destructure_own_line_comment_prettier_divergence/) — the same break for an object or array default, and the full position matrix
- [destructure_member_target](../destructure_member_target_prettier_divergence/) — another node the block-pattern printer throws on
