# inline_content_text_wrap_prettier_divergence

An inline element (`<small>`) whose **text content** overflows print width lays out **block-style**:
both tags stay intact and the content moves to its own indented line, exactly like a block element.
Content that fits stays inline (`<small>short text</small>`). The exact flip is pinned at the
100/101 boundary: a 100-char `<small>` of breakable words stays inline, one char longer (101) flips
to block-style.

```
<small>
	word word … word
</small>
```

Content-boundary whitespace is render-free under Svelte 5, so tsv places the block-style boundaries
freely. Prettier instead **dangles** the tag delimiters — pre-breaking the opening tag and dangling
the closing `>` — even for plain text; tsv keeps both tags intact and lays out block-style for
readability. `unformatted_ours_compact` (a single-line authoring) normalizes to this form under tsv
in one pass.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
