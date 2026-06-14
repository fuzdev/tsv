# comment_same_line_prettier_divergence

A `//` line comment that the author placed on the same line as the tag name or an
attribute stays trailing that token; prettier moves it to its own line.

tsv:

```svelte
<div // foo
	data-attr="value"
>…

<div
	data-attr1="value1" // mid
	data-attr2="value2"
>…
```

Prettier moves `// foo` and `// mid` onto their own lines:

```svelte
<div
	// foo
	data-attr="value"
>…

<div
	data-attr1="value1"
	// mid
	data-attr2="value2"
>…
```

Because a `//` comment runs to end of line, the following attribute (or the closing
`>` / `/>`) must drop to the next line either way — the only question is whether the
comment leads the next line or trails the previous token. tsv preserves where the
author wrote it; prettier always leads.

A `//` comment trailing the **last** attribute (before `>` / `/>`) stays inline in both
formatters, so that position is not a divergence — see
[comment_trailing_same_line](../comment_trailing_same_line/). Block comments and
own-line comments are preserved as-written by both.

## Reason

Comment placement is a deliberate authoring choice and tsv preserves it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [comment_trailing_same_line](../comment_trailing_same_line/) — same-line `//` trailing the last attribute (matches prettier, not a divergence)
- [comment](../comment/) — own-line attribute-list comments (matches prettier)
