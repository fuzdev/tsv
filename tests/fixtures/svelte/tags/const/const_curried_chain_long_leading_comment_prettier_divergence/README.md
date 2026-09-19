# const_curried_chain_long_leading_comment_prettier_divergence

A `{@const}` whose value is a curried arrow chain too long for its line takes the
`<script>` declarator's layout: the chain breaks after the `=` and stacks its heads at one
shared indent. **A comment on the `=` line, or glued to the first head, does not change
that** — the chain prints exactly as it does with no comment (`a`, the null control), the
comment leading the first head. The template twin of
[curried_chain_long_leading_comment](../../../../typescript/expressions/arrow/curried_chain_long_leading_comment_prettier_divergence/).

Prettier's answer changes with the comment's kind, as it does in a `<script>`:

- line comment (`b`) — Prettier **fabricates a blank line** below the comment and indents
  the chain two levels deeper
- single-line block (`c`) — Prettier cancels the break after `=` and indents the remaining
  heads under the first
- indentable block (`d`) — Prettier keeps the break and indents the whole chain one level
  deeper

A preserved multi-line block (`e`) leads the chain from the `=` line, and a comment on a
line of its own above the chain (`f`) takes the default chain shape — both are prettier's
forms too, pinned here as the boundary of the rule.

`unformatted_ours_heads_inline.svelte` writes every chain on one line, the authoring that
reaches the width-driven break; `unformatted_ours_operator_own_line.svelte` puts `b`'s `=` on
a line of its own below a blank, where "the comment is on the operator's line" has to be read
off the comment's own line rather than as a distance from the binding.

See [conformance_prettier_svelte.md](../../../../../../docs/conformance_prettier_svelte.md) §Svelte: Blocks,
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
