# Commented conditional alternate at an `{#each}` head with no `as`

The conditional twin of
[no_as_stripped_paren_comment](../no_as_stripped_paren_comment_prettier_divergence/): an
`{#each}` head with no `as` binding takes no comment at its value's end (Svelte reads its `}`
or `,` right behind the value), so a comment inside the author's pair around a conditional's
ALTERNATE — the operand the value ends on — has nowhere to go but that pair.

- **tsv**: keeps the alternate's pair, opened around a `//` (`c2`, `c7`, `c10`, `c11`) and
  holding a same-line block inline (`c3`, `c5`, `c8`, and `c14` behind a broken `?` line), at
  every depth of a conditional chain (`c7`), around a nested conditional that is itself the
  alternate (`c8`, `c11`), and ahead of the index's `, i` (`c5`, `c10`, whose `)` keeps the
  index on its line). The consequent's pair strips as anywhere else, since the `:` follows it
  (`c1`, `c4`, `c6`, `c9`, `c13`). A `// prettier-ignore` inside the pair freezes the alternate
  and stays in the pair with it (`c12`). Stripped, the alternate's comment would end the value
  — `{#each a⏎\t? b // c1⏎\t: c // c2⏎}` and `{#each a ? b : c /* c3 */}`, which Svelte
  rejects. The authored form (`unformatted_ours_inside_pair`) reaches `input.svelte` in one
  pass.
- **prettier**: strips the pairs and moves each `//` past the `}` into the block's body
  (`output_prettier.svelte`); a same-line block stays where its first pass leaves it, at the
  value's end, which it then refuses to parse.

See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
