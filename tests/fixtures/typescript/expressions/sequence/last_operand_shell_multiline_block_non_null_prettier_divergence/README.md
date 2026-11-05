# Sequence last operand's shell multi-line block, ahead of a non-null `!`

A multi-line block comment written last inside a redundant pair around a sequence's **last
operand** (`x = (a, (b /* x⏎ */))!`), where a non-null `!` follows the sequence: bare, as a
member's object (`!.m`, `![0]`), as a callee (`!()`), at a statement's start, in a `<script>`
and in a Svelte attribute value and mustache. A block whose own text spans a line carries a
`LineTerminator` for ASI (ecma262 sec-comments), and a postfix `!` must share its operand's
line, so the block cannot trail the sequence's `)`. tsv keeps a pair around the sequence and
the block, the form the bare spelling takes (`unformatted_ours_bare.svelte`):

```ts
x = (
	(a, b) /* x1
	 */
)!;
```

Authored (`unformatted_ours_shell.svelte`):

```ts
x = (a, (b /* x1
 */))!;
```

Prettier's first pass keeps the block inside the sequence's pair and breaks it after its comma
(`x = (a,⏎b /* x1⏎ */)!`); its next pass — and its first from `input.svelte` or the bare
spelling — prints the block after the `)`:

```ts
x = (a, b) /* x1
 */!;
```

**Prettier has no fixed point on this input**, so no prettier-anchored claim file is
expressible (`prettier_nonconvergent.txt`, live-verified by rule F5): every case of that
output but `![0]` (the statement start and the Svelte attribute and mustache included) is a
DEAD document prettier cannot re-read. The `![0]` case parses but as a different program —
the line break ends the statement at `(a, b)`, and `![0]` becomes a statement of its own.

The same block ahead of a binary operator or a sequence's `)` trails the `)`, where a line
break changes nothing:
[last_operand_shell_trailing_block](../last_operand_shell_trailing_block_prettier_divergence/).
The cast twin is
[last_operand_shell_multiline_block_cast](../last_operand_shell_multiline_block_cast_prettier_divergence/).

Reason: content integrity. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Redundant paren shell trailing block inside a pair the printer adds) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
