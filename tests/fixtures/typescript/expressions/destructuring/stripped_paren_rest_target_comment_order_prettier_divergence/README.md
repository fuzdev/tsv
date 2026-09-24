# Rest stripped-paren comment order divergence, assignment patterns

The destructuring-assignment twin of the spread case. A rest element's grouping
parens hold an own-line comment, and a comment is written after the `)`:

```js
[b, ...(a
/* i */
) /* t */] = x;
```

Prettier hoists the outside block onto the rest's line and drops the inside one
below it — the reverse of the order it was written:

```js
[
	b,
	...a /* t */
	/* i */
] = x;
```

tsv keeps the authored order, which is also what both formatters do with the
same rest written without the parens:

```js
[
	b,
	...a
	/* i */ /* t */
] = x;
```

The rest element shares the spread's stripped-paren partition, so it takes the
spread's answer, in the array and the object pattern alike: covered as the only
element and after another, in a nested pattern and a `for`-`of` head, with a
multi-line block inside the parens, with a line comment inside the parens (the
comment after the `)` then starts the next line, whether that line comment sits
on its own line or on the rest's — `[...(a // i⏎) /* t */] = x` →
`...a // i⏎/* t */`, a spelling prettier reverses too, though never without the
parens), and with two line comments (which prettier merges into one comment). A
block on its own line below a line comment's `)`, an order prettier keeps too,
is covered as a control.

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
