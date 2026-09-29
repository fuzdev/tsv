# Snippet parameter default, line comment inside the pair at the chain's end

The `{#snippet}`-head twin of
[right_operand_paren_line_comment](../../../../typescript/expressions/binary/right_operand_paren_line_comment_prettier_divergence/):
a default whose binary chain ends on a required pair holding a `//` (`a = x - (y - z // c⏎)`),
plain and inside a destructuring default. A snippet head formats exactly as its
`function` twin does, so it takes the same answer.

- **tsv**: keeps the comment inside the pair and opens it; the parameter list breaks
  around it.
- **prettier**: relocates the comment past the `)`; its second pass rejoins the chain
  (pinned in `audit_signature.txt`).

Left to defer, the comment would flush past the head's `)}` and come out as rendered
page text.

See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks),
which catalogs this fixture, and the rule it applies,
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
