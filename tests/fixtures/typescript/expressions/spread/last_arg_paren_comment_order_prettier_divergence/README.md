# Spread stripped-paren comment order divergence, last argument

When a spread is the **last** argument and its grouping parens hold an own-line
comment, that comment is the list's to print on a line of its own. A comment
written *after* the `)` then trails the same argument:

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
first — which is also what both formatters do with the same argument written
without the parens:

```js
fn(
	a,
	...b
	/* i */ /* t */
);
```

The same holds for every spelling of the pair: a trailing comma between them, a
line comment inside the parens (the comment after the `)` then starts the next
line, whether it is a block, which prettier hoists, or a line comment, which
prettier puts on the first one's line, merging the two into one comment — each
before or after a trailing comma), and a mixed inside run. A `//` written after
a block, and a block on its own line below the `)`, are orders prettier keeps
too, covered as controls; a line comment inside the parens followed by a comment
on its own line below the `)`, which prettier also keeps in order, is pinned in
[last_arg_paren_comment_own_line](../last_arg_paren_comment_own_line/). Covered
at a call, a `new` and a member-chain call.

A line comment the author wrote on the argument's own line inside the parens
rides that line, so a block written after the `)` follows it on the next one:
`fn(a, ...(b // i⏎) /* t */)` → `...b // i⏎/* t */`, with or without a trailing
comma, and behind a second, own-line `//` in the parens. Prettier reverses this
spelling too (`...b /* t */ // i`) — but only with the parens: without them, both
formatters keep the order, and tsv formats the parenthesized spelling the same
way.

Reason: comment order is comment position, and a pair of redundant parens
carries no authoring signal, so it may not decide the order. Every form is
stable in both formatters (`variant_hoisted.svelte` is prettier's), so the
divergence is only in which one the parenthesized authorings normalize to
(`unformatted_ours_parens.svelte`, and `unformatted_ours_glued.svelte` with the
`)` on the inside comment's line).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread or rest stripped-paren comment, then a comment after the `)`).
