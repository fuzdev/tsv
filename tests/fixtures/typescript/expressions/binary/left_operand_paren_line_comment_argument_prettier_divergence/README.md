# Binary chain's leftmost operand paren, line comment, at an argument

The mirror of
[right_operand_paren_line_comment](../right_operand_paren_line_comment_prettier_divergence/)
at the chain's other end: a `//` inside the grouping parens around a binary chain's
**leftmost** operand, where the chain is a call argument or an array element
(`h((// c⏎a || b) && c)`). Covers a first and a later argument, an array element, a
same-operator chain whose parens flatten (`(a || b) || c`), and a Svelte template
expression.

- **tsv**: strips the parens, hoists the comment ahead of the whole chain, and lays the chain
  out as the paren-free authoring (`h(⏎\t// c⏎\t(a || b) && c⏎)`) is: the comment breaks only
  its own line, never the chain. `unformatted_ours_stripped_shell.svelte` normalizes to
  `input.svelte` in one pass.
- **prettier**: its first pass binds the comment to the operand inside the chain's group, so
  the hard break it forces breaks every operator (`(a || b) &&⏎\t\tc`,
  `prettier_intermediate_stripped_shell.svelte`); its second pass binds the comment to the
  whole chain and rejoins it — `input.svelte`, a fixed point of both formatters.

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
