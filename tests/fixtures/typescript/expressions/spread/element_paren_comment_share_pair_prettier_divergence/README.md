# Spread stripped-paren comment pair divergence, mid-list

A spread's grouping parens hold two comments the author glued onto one line of
their own, on an element the list continues past:

```js
const a = [a, ...(b
/* i */ /* j */), c];
```

tsv gives each comment a line of its own:

```js
const a = [
	a,
	...b,
	/* i */
	/* j */
	c
];
```

Past the comma the next pass reads the pair as the next element's leading run,
where only a comment with a line of its own holds the list open — a comment glued
behind another does not, so a glued pair collapsed the list on the second pass.
On the last element nothing follows the pair, so it keeps its line (`a2`), and a
comment written after the `)` follows the pair, leading the next element.
Covered at a call, a `new`, an array literal and an object literal.

Prettier is not idempotent here: its first pass keeps the pair glued with the list
open (`prettier_intermediate_to_variant_parens.svelte`), and its second collapses
the argument lists and the array literals onto one line — the object literal stays
expanded, the pair glued on its own line (`variant_collapsed.svelte`), a form tsv
keeps too. The
divergence is only in which form the parenthesized authoring normalizes to
(`unformatted_ours_parens.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Spread or rest stripped-paren comment, then a comment after the `)`).
