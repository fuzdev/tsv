# Rest stripped-paren line-comment merge divergence, assignment patterns

The destructuring-assignment twin of the spread case. When a rest element's
grouping parens hold a **line** comment and another line comment is written
after the `)`, both trail that element:

```js
[...(a // c1
) // c2
] = x;
```

Prettier puts them on one line — which merges them into a **single** comment:
`// c1 // c2` reparses as one `//` whose text is ` c1 // c2`, so the second
comment stops existing.

```js
[
	...a // c1 // c2
] = x;
```

tsv gives the second one its own line, so both survive a reparse:

```js
[
	...a // c1
	// c2
] = x;
```

Reason: a `//` runs to end of line, so nothing may follow it on that line —
including a second deferred comment. The rest element shares the spread's
stripped-paren partition, so it takes the spread's answer, in the array and the
object pattern alike. Both forms are stable in both formatters
(`variant_merged.svelte` is prettier's), so the divergence is only in which one
the parenthesized authoring normalizes to (`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread stripped-paren line comment).
