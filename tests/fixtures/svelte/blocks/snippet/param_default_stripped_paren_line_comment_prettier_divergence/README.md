# Snippet parameter default, line comment inside a stripped pair

The `{#snippet}`-head twin of
[stripped_paren_trailing_line_comment](../../../../typescript/syntax/comments/stripped_paren_trailing_line_comment_prettier_divergence/):
a default whose `//` sits inside a grouping pair the printer strips — a parameter followed
by another (`v = x + (y * z // c1⏎), w`), the redundant outer layer of a double pair
(`x - ((y - z) // c2⏎)`), a destructuring default, and the run a sequence's own pair floats
out past its `)`. A snippet head formats exactly as its `function` twin does.

- **tsv**: one pass. The parameter list (or the destructuring pattern) breaks, and the
  comment flushes at the parameter's comma — ahead of the `, w` that follows it — inside
  the head.
- **prettier**: its first pass also splits the operand's own chain; its second pass rejoins
  it (`prettier_intermediate_inside_parens`).

See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks),
which catalogs this fixture, and the rule it applies,
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence).
