# Header test, comment in a stripped operand pair

A comment written last inside a redundant grouping pair down the right edge of a statement
header's test (`if (a + (b // c⏎)) {}`): a `//`, an own-line `//` or block, or a pair nested
in a pair the test keeps (`a + (b + (c // c⏎))`, where the comment lands past the kept
`(b + c)` as it does at a statement or an argument). The pair strips; the comment sat at the
test's right edge, inside the header's `(`, so it stays in the header after the test, which
breaks open around it — the form the bare spelling `if (a + b // c⏎) {}` takes in both
formatters (`unformatted_bare.svelte`):

```ts
if (
	a + b // c1
) {
}
```

- **tsv**: one pass to `input.svelte`.
- **prettier**: its first pass splits the test's binary chain further (`a +⏎\tb // c1`,
  `prettier_intermediate_shell.svelte`); its second pass undoes that, the same fixed point.

A run mixing a block and a `//` (`c7`), a comment after the header's `)` (`c8`) and a consequent
with no block (`c9`) take the same place. The multi-line block (`c6`) is the control: it already took this place in both formatters.

The twins at the other paren-headed statements:
[while](../../while/condition_stripped_operand_paren_comment_prettier_divergence/),
[do-while](../../do_while/condition_stripped_operand_paren_comment_prettier_divergence/),
[switch](../../switch/discriminant_stripped_operand_paren_comment_prettier_divergence/),
[with](../../../script_goal/sloppy_with_statement_stripped_operand_paren_comment_prettier_divergence/).

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
