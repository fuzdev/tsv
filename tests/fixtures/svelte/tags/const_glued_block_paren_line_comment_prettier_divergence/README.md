# const_glued_block_paren_line_comment_prettier_divergence

The `{@const}` home of
[glued_block_paren_line_comment](../../../typescript/syntax/comments/glued_block_paren_line_comment_prettier_divergence/):
a block comment glued to a redundant grouping `(` whose interior opens with a `//`, as a
`{@const}` declarator's value. tsv keeps the pair, laid out as a JSDoc cast's, so the block stays
glued to the `(` it was written against:

```svelte
{@const v = /* c1 */ (
	// d1
	x
)}
```

## Formatter divergence (`_prettier`) — prettier non-convergent

Prettier strips the pair and relocates the block ahead of the `=` (`{@const v /* c1 */ =`), the
declarator relocation it makes in a `<script>` too; in a `{@const}` tag that is not Svelte, so
prettier's own next pass throws on it. With no fixed point, no `output_prettier.svelte` is
expressible — `prettier_nonconvergent.txt` records the claim and the validator live-verifies it
(rule F5).

The second cell (`g1`) has the `//` glued to the `(`, which keeps that line — the
opening-delimiter rule. Prettier relocates its block ahead of the `=` the same way.

## Reason

**Comment position** (and `◆prettier_bug` for the unparseable output) — see
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(a block comment glued to a grouping paren whose interior opens with a line comment).
