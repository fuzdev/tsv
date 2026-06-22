# inline_element_long_prettier_divergence

A `{#snippet}` inside an inline element (`<A>text{#snippet name()}body{/snippet}</A>`) whose
construct exceeds printWidth.

tsv lays the inline element out **block-style**: both tags stay intact and the content (text +
snippet) moves to its own indented line when it overflows, collapsing back inline when it fits.
Prettier instead **dangles** the tags (`<A⏎\t>…content…</A⏎>`). `unformatted_ours_compact.svelte`
authors every case on one line: tsv normalizes it to the block-style `input` (content on its own
line where it overflows), while prettier dangles — so the **block-style layout is the divergence**.

```svelte
<!-- tsv (block-style) -->
<A>
	text{#snippet aaaa…()}x{/snippet}
</A>
```

The short cases (≤100 chars) stay inline in both formatters; only the overflowing cases drop the
content to its own line under tsv.

## Reason

tsv lays wrapping inline-element content out block-style (both tags intact) rather than prettier's
dangled tags — uniform with all other inline content, render-safe under Svelte 5. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

Note: when the snippet *name* alone overshoots printWidth (it is unbreakable), both an inline body
and a dropped body overshoot on the name line regardless, so whether the body drops or stays inline
is authoring-dependent — that body-drop convergence is folded into the deferred trailing/between-text
hug-convergence follow-up and is not covered here.

## Related

- [blocks/if/inline_element_long_prettier_divergence](../../if/inline_element_long_prettier_divergence/)
  — the same block-style layout inside an inline element, for `{#if}`
