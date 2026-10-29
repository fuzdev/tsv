# operand_left_spine_shell_block_comment_prettier_divergence

A block comment glued inside the stripped grouping shell at the head of a unary operand
(`typeof ((/* c */x)[y])`, `!((/* c */x).y)`). The operand's chain hoists the comment ahead
of its head, so the comment leads the operand's doc exactly as one written in the
operator→operand gap does (`typeof /* c */ x[y]`, the `unformatted_no_holder_pair` twin).

**tsv**: the comment-holder pair a leading comment earns is decided at the operand's
**printed** start rather than its span start, so the hoisted comment takes the pair in
**one** pass, at every unary operator:

```ts
typeof (/* c */ x[y]);
```

**Prettier**: reaches the same fixed point in two passes — its first prints the hoisted
comment with no pair (`typeof /* c */ x[y];`, the `prettier_intermediate_shell` file), and
its second, now reading the comment in the operator→operand gap, adds the pair. The
divergence is the pass count, not the destination.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
