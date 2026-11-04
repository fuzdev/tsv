# Cast operand shell trailing block inside the cast's operand pair

The cast twin of
[clarity_pair_operand_shell_trailing_block](../binary/clarity_pair_operand_shell_trailing_block_prettier_divergence/):
a same-line block comment written last inside a redundant grouping pair that ends an
`as` / `satisfies` operand, where the operand is a binary the cast prints its own pair
around (`(a - (b /* c2 */)) satisfies T` prints `(a - b) … satisfies T`). The author's pair
strips, or is the inner pair the operand keeps (`(a + (b + c /* c1 */)) as T`); either way
the comment sat at the operand's right edge, which the cast's `)` now closes, so it trails
that `)`, ahead of the keyword.

- **tsv**: one pass, `(a - b) /* c2 */ satisfies T` — at an assignment, a declarator init,
  a statement, a `return` argument, a nested call argument, an object value, an `if` test and
  a Svelte expression tag.
- **prettier**: its first pass prints the comment inside the cast's pair (`(a - b /* c2 */)
  satisfies T`), and its second moves it out — the same fixed point
  (`prettier_intermediate_shell`).

The pair written around the whole operand (`c7`) is the control: both formatters print it in
one pass.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
