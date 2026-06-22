# body_inline_content_prettier_divergence

A `{#snippet}` whose **body** is inline content (a `<span>`). When the body fits it stays flush on
the snippet line (`{#snippet badge()}<span>New</span>{/snippet}`); when it overflows, the body lays
out **block-style** — both tags intact, content on its own indented line, exactly like an inline
element elsewhere:

```
{#snippet row()}
	<span>
		…long inline content…
	</span>
{/snippet}
```

This is the snippet-body counterpart to `elements/inline_content_text_wrap` — the same block-style
content layout, here hosted inside a snippet block. Prettier instead **dangles** the tag delimiters
(`<span⏎\t>…</span⏎\t>`, hugging the snippet braces); tsv keeps both tags intact and lays out
block-style. The `unformatted_ours_compact` (single-line authoring) and `prettier_variant_compact`
(prettier's stable dangle) both normalize to `input.svelte` under tsv in one pass.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
