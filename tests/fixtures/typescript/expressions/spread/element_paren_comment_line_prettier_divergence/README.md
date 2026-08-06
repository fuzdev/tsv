# Spread stripped-paren line-comment merge divergence, list elements

The argument-list case's twin in the array and object literals. When a spread's
grouping parens hold a **line** comment and another line comment is written
after the `)`, both trail that element:

```js
const a = [...(b // i
) // t
, c];
```

Prettier puts them on one line — which merges them into a **single** comment:
`// i // t` reparses as one `//` whose text is ` i // t`, so the second comment
stops existing.

```js
const a = [
	...b, // i // t
	c
];
```

tsv gives the second one its own line, so both survive a reparse:

```js
const a = [
	...b, // i
	// t
	c
];
```

Reason: a `//` runs to end of line, so nothing may follow it on that line —
including a second deferred comment. The rule is about what a line comment does
to its line, so it holds in every list and at every position in one. Both forms
are stable in both formatters (`variant_merged.svelte` is prettier's), so the
divergence is only in which one the parenthesized authoring normalizes to
(`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread stripped-paren line comment).
