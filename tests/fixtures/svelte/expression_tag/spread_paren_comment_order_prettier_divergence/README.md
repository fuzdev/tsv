# Spread stripped-paren comment order divergence, template expression

The `<script>` rule in a template expression. A spread's grouping parens hold an
own-line comment, and a comment is written after the `)`:

```svelte
<p>{[a, ...(b
// i
) /* t */]}</p>
```

Prettier hoists the outside block onto the element's line and drops the inside
comment below it — the reverse of the order it was written:

```svelte
<p>
	{[
		a,
		...b /* t */
		// i
	]}
</p>
```

tsv keeps the authored order, as it does in a `<script>` and as both formatters
do with the same element written without the parens:

```svelte
<p>
	{[
		a,
		...b
		// i
		/* t */
	]}
</p>
```

A template expression is printed by the same list emitters as a `<script>`, so
it takes the same answer; the call argument (`{fn(a, ...(b⏎/* i */⏎) /* t */)}`)
is covered too. Every form is stable in both formatters (`variant_hoisted.svelte`
is prettier's), so the divergence is only in which one the parenthesized
authoring normalizes to (`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread or rest stripped-paren comment, then a comment after the `)`).
