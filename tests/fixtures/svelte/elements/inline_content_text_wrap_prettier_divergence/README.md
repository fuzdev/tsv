# inline_content_text_wrap_prettier_divergence

An inline element (`<small>`) whose **text content** overflows print width. tsv keeps the **opening
tag intact** and flows the text after it, wrapping at spaces — only the closing `>` dangles:

```
<small>word word … word
	word word word word</small
>
```

The principle: **text content has internal break points at every space**, so it flows like a
paragraph and needs no opening-`>` dangle. The dangle is reserved for *element/component* children
(atomic, whitespace-significant — where it avoids whitespace injection and communicates nesting).

Prettier instead pre-breaks the opening tag uniformly (`<small⏎\t>…content…</small⏎\t>`), even for
plain text — see `output_prettier.svelte`. tsv diverges here for readability.

`unformatted_ours_compact` (a single-line authoring) normalizes to this form under tsv in one pass.

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
