# Spread stripped-paren comment order divergence

When a spread is the **last** argument and its grouping parens hold an own-line
comment, a block written *after* the `)` joins it in trailing the argument:

```js
fn(a, ...(b
/* i */
) /* t */
);
```

Prettier hoists the outside block onto the argument's line and drops the inside
one below it, so the pair comes back in the reverse of the order it was written:

```js
fn(
	a,
	...b /* t */
	/* i */
);
```

tsv keeps the authored order — the comment written inside the parens stays
first:

```js
fn(
	a,
	...b
	/* i */ /* t */
);
```

Reason: comment order is comment position. Both forms are stable in both
formatters, so the divergence is only in which one the parenthesized authoring
normalizes to (`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread stripped-paren comment then an outside block).
