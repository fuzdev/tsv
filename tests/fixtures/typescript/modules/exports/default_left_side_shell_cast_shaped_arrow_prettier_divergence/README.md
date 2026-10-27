# default_left_side_shell_cast_shaped_arrow_prettier_divergence

JSDoc cast text inside the grouping parens of an `export default` value's **leftmost** node,
glued to an arrow function's parameter `(` (`export default (/** @type {T} */⏎(x) => zz) + q`).
The parser reads that `(` as the parameter list's, so there is no cast: the comment is an
ordinary block that ends its line, and the keyword→value seam hangs the value under the
keyword, as for any run that ends its line there
([default_left_side_shell_cast_shaped_block](../default_left_side_shell_cast_shaped_block_prettier_divergence/)).
The seam asks whether the parser built a cast for the comment, never its text.

The hung output wraps the arrow in a pair, which makes the comment a real cast the author gave
a line of its own — and the seam hangs that too, as every value gap does, so tsv lands on
`input.svelte` in one pass from the shell spelling (`unformatted_ours_inner_shell.svelte`):

```ts
export default
	/** @type {T} */
	((x) => zz) + q;
```

Prettier keeps the comment on the keyword's line and the value flush (`output_prettier.svelte`):

```ts
export default /** @type {T} */
((x) => zz) + q;
```

From the shell spelling prettier joins the two onto the keyword's line, a form both formatters
keep (`variant_inner_shell.svelte`).

The `export default` carve-out of the continuation-indent rule — prettier keeps the operand
break here, tsv hangs it indented. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
