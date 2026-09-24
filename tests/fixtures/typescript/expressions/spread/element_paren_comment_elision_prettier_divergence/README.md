# Spread stripped-paren comment order divergence, past an elision

The array literal's elision face of the spread's comment order. A spread's
grouping parens hold an own-line comment, a comment is written after the `)`,
and elisions follow the spread:

```js
const a = [...(b
/* i */) /* t */, // h
,];
```

tsv carries the comment the parens held and the one written after the `)`
together past the elision commas, in the order the author wrote them — the same
forward slide every other comment in a hole region takes:

```js
const a = [
	...b,
	,
	/* i */ /* t */ // h
];
```

With only holes after the spread the pair reaches the array's trailing position,
ahead of a line comment written there (`a2`, `a3`) or a space apart from a block
(`a4`); with a real element after the
holes the block written after the `)` leads it (`a1`). Either way the carried form
is the one the array reprints once the parens are gone.

Prettier pulls the comments back before the elision comma instead
(`output_prettier.svelte`) — the output divergence cataloged as the array
trailing-elision block comment (see below). From the parenthesized authoring it
first hoists the outside block onto the spread's line
(`prettier_intermediate_to_divergent_variant_parens.svelte`), then settles on a
form whose own-line comments tsv carries past the elision comma again
(`divergent_variant_pulled_back.svelte`).
Which form the parenthesized authoring normalizes to is this fixture's divergence
(`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread or rest stripped-paren comment, then a comment after the `)`; Array
**trailing-elision** block comment).
