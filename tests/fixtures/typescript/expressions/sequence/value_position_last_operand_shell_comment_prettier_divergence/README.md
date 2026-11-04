# Sequence value position, comment in a shell down the last operand

A comment written last inside a redundant grouping pair down a sequence's last operand — on
its right spine (`x = (a, b < (c /* c1 */))`) or in a nested sequence's last operand
(`const y = (a, (b, (c /* c2 */)))`) — at a value position, where the sequence's own pair
keeps a trailing comment inside it (prettier #19263): an assignment, a declarator
initializer (a later declarator too, `c4`), an arrow body. The author's pair strips, and the
comment stays inside the sequence's pair, as the bare spelling `(a, b < c /* c1 */)` keeps it.

- **tsv**: one pass. A `//` hangs the arrow's pair and keeps the comment inside it (`c5`), the
  bare spelling's form.
- **prettier**: the same fixed point. Where the comment sits in a nested sequence (`c2`) or
  is a `//` (`c5`), its first pass lands elsewhere (`prettier_intermediate_shell`) and its
  second reaches `input.svelte`'s form.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Sequence trailing line-comment convergence).
