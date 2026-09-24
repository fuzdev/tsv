# Spread stripped-paren comment order divergence, mid-list

The last-argument case's twin, at an argument the list continues past. The
spread's grouping parens hold an own-line comment, and a block is written after
the `)`, before the comma:

```js
fn(a, ...(b
/* i */
) /* t */, c);
```

Prettier hoists the outside block onto the argument's line, ahead of the comma,
and drops the inside one below it — the reverse of the order it was written:

```js
fn(
	a,
	...b /* t */,
	/* i */
	c
);
```

tsv keeps the authored order: the comma moves ahead of both comments — as it does
for an own-line comment the author left before a comma (a same-line
`x /* t */, c` keeps its place) — the inside comment keeps its own line, and the
block leads the next argument:

```js
fn(
	a,
	...b,
	/* i */
	/* t */ c
);
```

The same holds when the parens hold a line comment (`...(b⏎// i⏎) /* t */, c` →
`...b,⏎// i⏎/* t */ c`), and for a line comment after the comma
(`...(b⏎// i⏎), // t⏎c` → `...b,⏎// i⏎// t⏎c`), where prettier instead puts it
on the inside comment's line, merging the two into one comment. A line comment
on the argument's own line inside the parens rides that line past the comma,
and the block after the `)` leads the next argument (`...(b // i⏎) /* t */, c` →
`...b, // i⏎/* t */ c`) — a spelling prettier reverses too (`...b /* t */, // i`),
though never without the parens. A comment written after the comma, or on its
own line below the `)`, which prettier also keeps in order, is covered as a
control. Covered at a call, a `new` and a member-chain call.

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
