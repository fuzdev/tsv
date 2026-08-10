# no_as_key_comment_svelte_prettier_divergence

A comment inside the **key parens** of an `{#each}` head that has no `as` binding —
`{#each items, i (…)}` — is preserved where the author wrote it, exactly as it is in the
ordinary `{#each … as item, i (…)}` shape.

tsv: `{#each items, i (i /* c */)}` (preserved, once)
Prettier: `{#each items, i (i)}` (comment dropped)

Covered positions (block comments, head stays inline): leading inside the key parens
(`(/* c */ i)`, preserved by both formatters) and trailing inside them (`(i /* c */)`,
the divergence). Each is paired with its `as`-binding counterpart, because the parity of
the two head shapes is the point: the no-`as` form is the one whose iterable has no
binding pattern after it, so nothing but the key itself bounds the region the head's own
trailing-comment scan may claim, and the scan used to run all the way to the `}` — over
the whole `, index (key)` tail — printing the key's comment a second time after the
iterable (`{#each items /* c */, i (i /* c */)}`, which neither parser accepts).

Every other position in the no-`as` tail — between the iterable and the `,`, around the
index, and after the `)` — is a parse error in canonical Svelte (and in tsv), so the key
parens are the only comment-bearing gap the tail has.

## Svelte divergence (parser)

Canonical Svelte reads the no-`as` head **twice**. `read_expression` parses
`items, i (/* c */ i)` as one sequence expression (collecting the key's comment), then the
`{#each}` reader discards everything past the first operand and rewinds `parser.index` to
the iterable's end so the `, index (key)` tail can be read properly — and the tail's own
`read_expression` collects the same comment again. Both copies land in the shared
`root.comments` array and both are attached to the key node, so canonical lists a no-`as`
key comment twice where tsv lists it once. The `as`-bound counterparts in this fixture are
read once and match. Same family as the other canonical comment-glue duplications. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
here. prettier-plugin-svelte prints the key from a comment-blind path and drops a comment
that trails the key expression. See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — the same
  trailing-comment rule across the `{…}` contexts, including the `as`-bound each key
- [expr_block_each](../../../syntax/comments/expr_block_each/) — the leading positions both
  formatters preserve
- [no_as_with_index_key](../no_as_with_index_key/) — the comment-free head shape
