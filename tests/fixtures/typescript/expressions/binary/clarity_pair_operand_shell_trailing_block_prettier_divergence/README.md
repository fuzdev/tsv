# Operand shell trailing block inside a clarity pair the printer adds

A same-line block comment written last inside a redundant grouping pair (`(b /* c */)`) that
ends an operand, where the operand around it takes a **clarity pair the printer adds** — a
mixed-precedence binary operand (`a >> (b /* c1 */) >> d` prints `(a >> b) … >> d`), a `??`
operand over `&&`. The author's pair strips; the comment sat at
the operand's right edge, which the added pair's `)` now closes, so it trails that `)`.

- **tsv**: one pass, `(a >> b) /* c1 */ >> d` — at a statement, an assignment, a declarator
  init, a `return` argument, a nested call argument, an object value, a `for` init, an `if`
  test and a Svelte expression tag. A block the author wrote after their pair (`/* e */`,
  beside `c11`) stays behind the moved one, in source order, and a multi-line block (`c13`)
  trails the pair the same way, since a line break ahead of an operator is harmless. With
  the author's pair around the whole statement too (`c14`), that pair strips and the
  comment trails the statement past its `;`.
- **prettier**: its first pass prints the comment inside the added pair (`(a >> b /* c1 */)
  >> d`); the reparse reads it in that pair's trailing gap, and its second pass moves it out
  to `input.svelte`'s form — the same fixed point (`audit_signature_shell.txt`).

The pair written around the whole operand (`c8`) and a right-nested power whose comment
leaves the statement (`c9`, past the `;`, where prettier takes three passes) are the
controls: tsv prints both in one pass, the comment where the statement's own tail takes it.

The cast's twin is
[as_satisfies_operand_pair_shell_trailing_block](../../as_satisfies_operand_pair_shell_trailing_block_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
