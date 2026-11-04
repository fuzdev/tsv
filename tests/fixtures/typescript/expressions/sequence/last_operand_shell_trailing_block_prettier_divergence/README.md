# Sequence last operand's shell trailing block, after the sequence's own pair

A block comment written last inside a redundant grouping pair at the right edge of a
sequence's **last operand** (`f((a, b < (c /* c2 */)))`), at a position where the sequence
prints its own pair and its trailing comments float out past it: a call argument, a
statement header's test, a ternary's test, an object value. The author's pair strips; the
comment sat at the last operand's right edge, which the sequence's `)` now closes, so it
trails that `)`. Nested sequences take the same place, the outermost `)` (`c6`), and so do a
multi-line block (`c7`, `c8`) and a run of blocks on the operand's line (`c9`). At a
statement the comment trails the statement past its `;`, as the bare spelling's does, with
or without the author's pair around the whole sequence (`c10`–`c12`).

- **tsv**: one pass, `f((a, b < c) /* c2 */)`.
- **prettier**: its first pass prints the comment inside the sequence's pair (`(a, b < c
  /* c2 */)`); the reparse reads it in that pair's trailing gap, and the next pass moves it
  out to `input.svelte`'s form, the same fixed point (`audit_signature_shell.txt`).

A pair around the last operand alone (`c1`) is the control: tsv already printed it in one
pass through the operand's own shell emitter; prettier takes the same two passes there.

The binary operand's and the cast's twins are
[clarity_pair_operand_shell_trailing_block](../../binary/clarity_pair_operand_shell_trailing_block_prettier_divergence/)
and
[as_satisfies_operand_pair_shell_trailing_block](../../as_satisfies_operand_pair_shell_trailing_block_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
