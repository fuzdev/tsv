# value_paren_trailing_block_comment_prettier_divergence

A redundant paren shell around a **value** whose trailing gap holds a block comment
(`const a = (x /* t */);`), at every `;`-terminated value position: a declarator
initializer, an assignment RHS, a compound assignment, a `return` argument, a `throw`
argument, and `export default`.

The `throw` and `export default` cells state the boundary of the one carve-out: a comment
stays *inline before the `;`* only when a `)` the OUTPUT prints still encloses it. That is
true of a `return` argument's retained parens
([operand_paren_assignment_comment](../../../statements/return_throw/operand_paren_assignment_comment/))
and false of the clarity pair a `throw` operand or an `export default` value takes — those
close at the expression, leaving the gap outside them. Keying the carve-out on the *source*
shell's `)` instead cost both positions their one-pass answer (pass 1 held the comment at
`throw (c = d) /* t */;` / `export default (e = f) /* t */;`), which is exactly the
fixed-point failure `split_terminator_gap_comments` warns of — and at `throw` it left the
statement with **two** fixed points, one per authoring, which F1 cannot see.

**tsv**: strips the shell and defers the trailing block past the statement `;` (via
`line_suffix`) — the declarator's own value-to-`;` trailing-comment handling — reaching
the fixed point in **one pass**, from either authoring:

```ts
const a = x; /* t */
```

**Prettier**: reaches the same fixed point but is **non-idempotent** on the paren
shell — its first pass lands the block before the `;` (`const a = x /* t */;`, the
`prettier_intermediate_*` files) and its second pass moves it past the `;`
(`input.svelte`). The divergence is the pass count, not the destination.

This is the value-position twin of
[as_satisfies_value_trailing_block_comment](../../../expressions/as_satisfies_value_trailing_block_comment_prettier_divergence/),
which answers the same question for a cast type's shell, and of the class field
`f = (x /* t */)`, which was already one-pass. A **line** comment in the same gap is a
different rule entirely: it cannot defer past the `;` without re-binding to the
statement, so the shell is **retained** and the comment stays inside it — see
[init_assignment_paren_line_comment](../../../statements/variable/init_assignment_paren_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
