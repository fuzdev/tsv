# default_value_glued_block_paren_line_comment_prettier_divergence

`export default` and `export =` with a block comment glued to the value's redundant grouping
`(`, and a `//` opening the pair's interior. Stripping the pair would put the `//` behind the
block on the keyword's line, so the block would lead nothing; tsv keeps the pair and the block
stays glued to its `(`:

```ts
// tsv                                   // prettier
export default /* @__PURE__ */ (         export default /* @__PURE__ */ // side-effect free
	// side-effect free                    fn();
	fn()
);
```

On prettier's form a bundler no longer reads the annotation as marking the call (rollup keeps
the call). The same rule at every other expression position is
[glued_block_paren_line_comment](../../../syntax/comments/glued_block_paren_line_comment_prettier_divergence/).

Written glued to the `(`, the `//` keeps that line —
[default_value_glued_block_paren_line_comment_glued](../default_value_glued_block_paren_line_comment_glued_prettier_divergence/).

## Variants

- `unformatted_ours_paren_blank.svelte` — a blank between the `(` and the `//`; it sits against
  the delimiter and is erased, as at every bracket.
- `unformatted_ours_nested_paren.svelte` — two redundant pairs, which strip as a unit to one.

## Reason

**Comment position** — a glued block comment binds to the value it leads, and a stripped pair
must not let another comment land between them. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(a block comment glued to a grouping paren whose interior opens with a line comment).
