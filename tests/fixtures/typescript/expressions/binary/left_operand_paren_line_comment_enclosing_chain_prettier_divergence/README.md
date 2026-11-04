# Binary chain's leftmost operand paren, line comment, under an enclosing chain

The enclosing-chain sibling of
[left_operand_paren_line_comment_argument](../left_operand_paren_line_comment_argument_prettier_divergence/):
a `//` inside the grouping parens around the leftmost operand of an **inner** chain whose
enclosing chain of a lower precedence prints no pair around it (`(// c⏎a) >> b < c`, where
`a >> b` is the left operand of `<`). Covers a statement, an assignment, a declarator init, a
`return` argument, a nested call argument, an object value, a call / member / tagged-template
base, and an own-line or a multi-line block, which breaks its line the same way.

- **tsv**: strips the parens, hoists the comment ahead of the whole value, and lays both
  chains out as the paren-free authoring is (`// c⏎a >> b < c`): the comment breaks only its
  own line, never the enclosing chain — outside a statement header's test, where the chain
  prints in the header's own group and breaks in both formatters (`c12`, a control: tsv's one-pass form is prettier's fixed point). `unformatted_ours_stripped_shell.svelte` normalizes to
  `input.svelte` in one pass.
- **prettier**: its first pass binds the comment to the operand inside the chains' groups, so
  the hard break it forces breaks every operator (`a >>⏎\t\tb <⏎\t\tc`,
  `prettier_intermediate_stripped_shell.svelte`); its second pass rejoins them —
  `input.svelte`, a fixed point of both formatters.

A glued single-line block (`c11`) breaks no line, so it stays inline ahead of the value in
both formatters, in one pass.

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
