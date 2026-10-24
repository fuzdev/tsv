# default_value_glued_block_paren_line_comment_glued_prettier_divergence

[default_value_glued_block_paren_line_comment](../default_value_glued_block_paren_line_comment_prettier_divergence/)
with the `//` glued to the kept pair's `(`. The pair is kept for the same reason, and the
`//` keeps the `(` line it was written on — the opening-delimiter rule, as at `return (` and a
JSDoc cast's `(`:

```ts
// tsv                                                 // prettier
export default /* @__PURE__ */ ( // side-effect free   export default /* @__PURE__ */ // side-effect free
	fn()                                                 fn();
);
```

Its own fixture rather than more cells in the sibling's input because a module holds one
`export default`, and a second `<script module>` would carry its comments onto the instance
script's first statement in Svelte's own parse (a parser divergence of its own).

## Reason

**Comment position** — a glued block comment binds to the value it leads, and a line comment
the author put on an opening delimiter's line stays there. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(a block comment glued to a grouping paren whose interior opens with a line comment, and a
comment-bound pair's `(`).
