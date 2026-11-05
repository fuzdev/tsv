# Sequence last operand's shell multi-line block, ahead of a cast keyword

A multi-line block comment written last inside a redundant pair around a sequence's **last
operand**, where `as` / `satisfies` follows the sequence and the sequence prints its own pair
(`void (a, (b /* x⏎ */)) as T`, `as const` included). A block whose own text spans a line
carries a `LineTerminator`, and the keyword must share the `)`'s line, so the block cannot trail
the sequence's `)`. tsv keeps a pair around the sequence and the block in one pass, the form the
bare spelling takes in both formatters (`unformatted_bare.svelte`):

```ts
void (
	(a, b) /* x1
	 */
) as T;
```

Authored (`unformatted_ours_shell.svelte`):

```ts
void (a, (b /* x1
 */)) as T;
```

- **tsv**: one pass to `input.svelte`.
- **prettier**: its first pass keeps the block inside the sequence's pair and breaks the
  sequence after its comma (`void (a,⏎b /* x1⏎ */) as T`, `prettier_intermediate_shell.svelte`);
  its second pass reads the block in that pair's trailing gap and settles on `input.svelte`,
  the same fixed point.

The non-null twin, where prettier prints the block past the `)` and so cannot re-read its
own output, is
[last_operand_shell_multiline_block_non_null](../last_operand_shell_multiline_block_non_null_prettier_divergence/).

Reason: pass count — prettier reaches the same fixed point in two passes. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Redundant paren shell trailing block inside a pair the printer adds).
