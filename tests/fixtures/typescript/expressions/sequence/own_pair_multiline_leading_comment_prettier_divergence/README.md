# own_pair_multiline_leading_comment_prettier_divergence

A multi-line comment the author wrote **inside** a sequence's own pair, at the positions where
the sequence prints that pair itself: the `export default` value and a later operand of another
sequence.

tsv (`input.svelte`) keeps it inside the pair:

```ts
export default (/* c
 * d */ b, c);
```

Prettier hoists it in front of the pair (`output_prettier.svelte`):

```ts
export default /* c
 * d */ (b, c);
```

The pair is required, so this is the required-pair rule — a comment written inside the pair
stays there, and a `@type` block hoisted in front of a `(` would become a JSDoc cast. Both
positions build their value with the claim that prints a multi-line comment outside the
value's own group; a sequence declines it and claims inside its own pair instead, so the
operands stay flat either way.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Leading comment inside a REQUIRED paren pair, **multi-line** block).
