# Comment in a stripped shell around a chain's callee or `!` operand

Grouping parens the parser strips from around a CALL's callee or a `!`'s operand
(`(/* c1 */ (x.y).p)()`), in a chain where a member below holds a stripped shell
of its own. The `unformatted_ours_stripped_shell` variant holds those authorings;
`input.svelte` is where tsv prints them, in one pass.

- **tsv**: the comment leads the chain, where the author wrote it — the same place
  the bare spelling `/* c1 */ x.y.p();` keeps it. Such a shell belongs to no member
  access, so unlike a shell around a member's object it widens no member's gap
  ([block_comment_chain](../block_comment_chain_prettier_divergence/), whose
  comment lands mid-chain). It holds for a call (c1, c2), a `!` (c3, c4), a sealed
  base (c5), a line comment (c6), and a callee shell with no shell inside it
  (c10, the control). Nested with object shells (c7–c9), the object shells'
  comments lead the chain too, so all three keep the authored order
  (`/* c7 */ /* c8 */ /* c9 */ x.y.p().q();`): an object shell's comment lands
  mid-chain only while that keeps the order
  ([stripped_shell_comment_source_order](../stripped_shell_comment_source_order_prettier_divergence/)).
  Prettier swaps them (`x.y⏎\t/* c9 */ .p /* c8 */⏎\t()⏎\t/* c7 */ .q();`).
- **prettier**: attaches the comment to the callee member and prints it mid-chain
  on its own line (`x.y⏎\t/* c1 */ .p();`), and on its second pass moves c5 into
  the brackets (`(x?.y).s[/* c5 */ 0]();`). At the `!` (c3) it leads the chain,
  as in tsv. Every line of `input.svelte` is a prettier fixed point, so the
  divergence is the normalization alone.

```
(/* c1 */ (x.y).p)();   →  tsv:      /* c1 */ x.y.p();
                           prettier: x.y
                                     	/* c1 */ .p();
```

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
