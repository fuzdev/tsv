# type_operator_keyword_own_line_block_comment_prettier_divergence

A single-line block comment between a prefix type operator
(`keyof`/`typeof`/`readonly`) and its operand collapses to the inline form
(`keyof /* a */ B`) — whether the author wrote it glued, trailing the operator, or
on its own line. The own-line authoring (`type A = keyof⏎/* a */⏎B`,
`unformatted_ours_own_line.svelte`) is what tsv normalizes here.

- **tsv** collapses the operand break in one pass: `type A = keyof /* a */ B;`.
- **Prettier** reaches the same inline form but is **non-idempotent** — its first
  pass pulls the comment onto the operator line yet leaves the operand on the next
  line (`keyof /* a */⏎B`), collapsing fully only on a second pass. That unstable
  first pass is pinned by `prettier_intermediate_own_line.svelte` (the validator
  confirms it reconverges to `input`).

Block comments inline losslessly, so the collapse never drops information; the
prefix operators are an *in-place-collapse* gap (prettier keeps the comment after
the operator rather than relocating it). Only a **line** comment (which can't
inline — [type_operator_keyword_line_comment](../type_operator_keyword_line_comment_prettier_divergence/))
or a **multiline** block comment still hangs the operand on its own line. Covers
`keyof`, `typeof` (a `TypeQuery` node), and `readonly`.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
