# Spread stripped-paren comment order divergence, list elements

The argument-list cases' twin in the array and object literals. A spread's
grouping parens hold an own-line comment, and a comment is written after the
`)`:

```js
const a = [a, ...(b
/* i */
) /* t */
];
```

Prettier hoists the outside block onto the element's line and drops the inside
one below it — the reverse of the order it was written:

```js
const a = [
	a,
	...b /* t */
	/* i */
];
```

tsv keeps the authored order, which is also what both formatters do with the
same element written without the parens:

```js
const a = [
	a,
	...b
	/* i */ /* t */
];
```

Covered in both literals. At the last element: with and without a trailing
comma between the pair; with a line comment inside the parens, after which the
comment from past the `)` starts the next line, before or after a trailing
comma, whether that line comment sits on its own line or on the element's
(`...(b // i⏎) /* t */` → `...b // i⏎/* t */`, a spelling prettier reverses too,
though never without the parens); with two line comments, which prettier merges
into one comment; and with a mixed inside run. At an element the list continues
past, the comma moves ahead of both comments and the one after the `)` leads the
next element (`...(b⏎/* i */⏎) /* t */, c` → `...b,⏎/* i */⏎/* t */ c`,
`...(b // i⏎) /* t */, c` → `...b, // i⏎/* t */ c`). A block on its own line
below a line comment's `)`, an order prettier keeps too, is covered as a
control.

Reason: comment order is comment position, and a pair of redundant parens
carries no authoring signal, so it may not decide the order. Every form is
stable in both formatters (`variant_hoisted.svelte` is prettier's), so the
divergence is only in which one the parenthesized authoring normalizes to
(`unformatted_ours_parens.svelte`, and `unformatted_ours_glued.svelte` with the
`)` on the inside comment's line).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread or rest stripped-paren comment, then a comment after the `)`).
