# default_left_side_shell_cast_shaped_block_prettier_divergence

JSDoc cast text inside the grouping parens of an `export default` value's **leftmost** node,
glued to a name rather than to a `(` (`export default (/** @type {T} */⏎f)(() => {…})`). A cast
is the comment plus the `(` it glues to, so this one is an ordinary block: the value's printer
hoists it ahead of its doc, it ends its line, and the keyword→value seam hangs the value under
the keyword, as it does for any run that ends its line there
([default_left_side_shell_comment](../default_left_side_shell_comment_prettier_divergence/)).
Read as a cast, the value printed flush below the keyword and the reparse indented it.

tsv (`input.svelte`):

```ts
export default
	/** @type {T} */
	f(() => {
		q();
	});
```

Prettier keeps the comment on the keyword's line and the value flush (`output_prettier.svelte`):

```ts
export default /** @type {T} */
f(() => {
	q();
});
```

The `export default` carve-out of the continuation-indent rule — prettier keeps the
operand break here, tsv hangs it indented. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
