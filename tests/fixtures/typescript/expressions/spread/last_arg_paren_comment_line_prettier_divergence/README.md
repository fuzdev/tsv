# Spread stripped-paren line-comment merge divergence

When a spread's grouping parens hold a **line** comment and another line comment
is written after the `)`, both trail the last argument:

```js
fn(a, ...(b // i
) // t
);
```

Prettier puts them on one line — which merges them into a **single** comment:
`// i // t` reparses as one `//` whose text is ` i // t`, so the second comment
stops existing.

```js
fn(
	a,
	...b // i // t
);
```

tsv gives the second one its own line, so both survive a reparse:

```js
fn(
	a,
	...b // i
	// t
);
```

Reason: a `//` runs to end of line, so nothing may follow it on that line —
including a second deferred comment. Both forms are stable in both formatters
(`variant_merged.svelte` is prettier's), so the divergence is only in which one
the parenthesized authoring normalizes to (`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread stripped-paren line comment).
