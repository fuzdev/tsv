# pre_block_pattern_comment_prettier_divergence

A comment that breaks a block binding pattern, inside a whitespace-sensitive element (`<pre>`):
the `{#each}` context, the `{#await}` then-shorthand and catch-shorthand bindings, the
`{:then}` and `{:catch}` bindings, and a `{#snippet}` parameter.

The pattern lays out like its TypeScript twin, as it does anywhere else — but here the break
lands inside the braced head, where no character is content, so the render is unchanged, and
the block's sections do **not** expand: a line break beside a section body is rendered
content in a whitespace-sensitive element, so every body stays glued to the tags around it
exactly as authored.

tsv:

```svelte
<pre>{#await promise then value}{value}{:catch {
		e
		/* c */
	}}{e}{/await}</pre>
```

Prettier drops the comment: `<pre>{#await promise then value}{value}{:catch { e }}{e}{/await}</pre>`.

`unformatted_ours_inline.svelte` holds the inline authorings.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [pre_block_head_line_comment](../pre_block_head_line_comment_prettier_divergence/) — a `//` in the head's own expression, at every block the whitespace-sensitive builders own
- [each/destructure_own_line_comment](../../blocks/each/destructure_own_line_comment_prettier_divergence/) — the broken layout outside a whitespace-sensitive element, where the block expands
