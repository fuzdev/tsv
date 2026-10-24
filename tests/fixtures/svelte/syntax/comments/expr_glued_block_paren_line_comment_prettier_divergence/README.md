# expr_glued_block_paren_line_comment_prettier_divergence

The template-expression twin of
[glued_block_paren_line_comment](../../../../typescript/syntax/comments/glued_block_paren_line_comment_prettier_divergence/):
a block comment glued to a redundant grouping `(` whose interior opens with a `//`, in an
`{expr}` tag, an attribute value, an `{#if}` head, `{@html}` and a `??` operand. tsv keeps the
pair, so the block stays glued to the `(` it was written against.

```svelte
<!-- tsv -->           <!-- prettier -->
<p>                    <p>
	{/* c1 */ (            {// d1
		// d1              a}
		a              </p>
	)}
</p>
```

Prettier strips the pair and **drops** the block at every braced head (`c1`–`c4`); at the `??`
operand it keeps it, behind the operator with the `//` on its line.

The cells after the second comment repeat each one with the `//` glued to the `(`
(`g`/`h` for `c`/`d`), which keeps that line — the opening-delimiter rule. Authored that way,
prettier moves the `??` operand's block past the `//` instead, onto the value's line
(`h6`); `divergent_variant_glued_line.svelte` is that form on its own, which prettier holds
and tsv formats to its own stripped layout.

## Variants

- `unformatted_ours_paren_blank.svelte` — a blank between the `(` and the `//`, erased against
  the delimiter.
- `unformatted_ours_nested_paren.svelte` — two redundant pairs, printed as one.

## Reason

**Comment position** (and `◆content_preservation` for the dropped comment) — see
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(a block comment glued to a grouping paren whose interior opens with a line comment).
