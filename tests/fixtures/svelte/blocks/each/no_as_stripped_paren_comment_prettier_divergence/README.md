# Stripped operand paren at an `{#each}` head with no `as`

An `{#each}` head with no `as` binding is the one braced head whose value may not END on a
comment: Svelte reads its `}` or `,` right behind the value, and rejects a comment in
between (`{#each a /* c */}` and `{#each a /* c */, i}` are both syntax errors, while
`{#each a || (b /* c */)}` is not). So a comment inside a grouping pair the printer would
strip — a redundant pair around a chain's last operand, one a rebalanced chain flattens,
the body of a sole hugged arrow — has nowhere to go but that pair.

- **tsv**: keeps the author's pair and opens it around a `//` or an own-line comment
  (`c1`–`c4`), as it opens a pair the chain's last operand REQUIRES
  (`{#if x - (⏎\t\ty - z // c⏎\t)⏎}`); a same-line block stays inline inside it (`c5`).
  Stripped, the comment would end the value — `{#each a || b // c1⏎}`, which Svelte
  rejects. The pair is kept whatever follows it in the value — a member (`c7`), an index
  (`c8`) — and of nested redundant pairs the outermost is kept, holding both comments
  (`c9`, `c10`), while a pair holding no comment strips even when it ends a chain (`c11`).
  A pair followed by an argument strips as anywhere else, its comma ending
  the line (`c6`). The stripped-pair authoring (`unformatted_ours_inside_pair`) reaches
  `input.svelte` in one pass.
- **prettier**: strips the pair and moves each `//` past the `}` into the block's body
  (`output_prettier.svelte`); from the authored form its first pass prints
  `{#each a || b /* c5 */}`, which it then refuses to parse, so no chain signature exists.

See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The rule at every other island closer is
[binary_stripped_operand_paren_line_comment_island_end](../../../expressions/binary_stripped_operand_paren_line_comment_island_end_prettier_divergence/).
