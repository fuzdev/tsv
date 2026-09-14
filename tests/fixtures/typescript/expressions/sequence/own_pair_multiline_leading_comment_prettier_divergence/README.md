# own_pair_multiline_leading_comment_prettier_divergence

A multi-line comment the author wrote **inside** a sequence's own pair, at the positions where
the sequence prints that pair itself: the `export default` value, a later operand of another
sequence, and every value seam whose leading run is hoisted out of the value's group — the
declarator, the assignment, the class field, the binding default, the enum member, the object
property and the arrow body.

tsv (`input.svelte`) keeps it inside the pair:

```ts
export default (/* c
 * d */ b, c);
const v1 =
	(/* c
	 * d */ b, c);
```

Prettier hoists it in front of the pair (`output_prettier.svelte`):

```ts
export default /* c
 * d */ (b, c);
const v1 =
	/* c
	 * d */ (b, c);
```

The pair is required, so this is the required-pair rule — a comment written inside the pair
stays there. The value seams hoist a multi-line run out of the value's own group so the
operands stay flat, and that hoist declines at a pair the printer re-emits: the comment is
claimed inside the sequence's own envelope instead, outside the operand run's group, so the
operands stay flat either way. The single-line kind (`v2`) takes the same place.

What takes the rule past taste is `checkJs`: a `/** @type {T} */` block hoisted in front of a
`(` is a JSDoc cast tsc reads, where the authored form is a comment and nothing more.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Leading comment inside a REQUIRED paren pair, **multi-line** block).
