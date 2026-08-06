# Spread stripped-paren interior, then a line comment in the gap

A spread's grouping parens hold an own-line comment, and a line comment is
written after the `)` — before the separator comma:

```js
const a = [...(b
/* i */
)// c
, d];
```

Prettier keeps the two in the order they were written, gluing the `//` onto the
inside comment's line:

```js
const a = [
	...b,
	/* i */ // c
	d
];
```

tsv places each by its own rule, and the two rules disagree about which comes
first:

```js
const a = [
	...b, // c
	/* i */
	d
];
```

Reason: a comment between an element and its comma trails that element past the
pure separator — the sanctioned trail
([conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)),
and the only rendering a `//` has there, since it runs to end of line. The
comment the parens held is the *list's* share, not the spread's, and needs a
line of its own below (`docs/comments.md` §A stripped-paren interior is a
partition too). Neither placement moves a comment across a boundary; their
output order is a consequence of the two, not a relocation of either. Uniform
across the argument list, the object literal and the array literal. Both forms
are stable in both formatters (`variant_glued.svelte` is prettier's), so the
divergence is only in which one the parenthesized authoring normalizes to
(`unformatted_ours_parens.svelte`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread stripped-paren interior then a gap line comment).
