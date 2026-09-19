# const_curried_head_break_leading_comment_prettier_divergence

A `{@const}` whose value is a curried arrow chain whose **heads force the break** (a
non-identifier parameter, `({}) =>`) takes the `<script>` declarator's layout: the chain
breaks after the `=`, however short it is, and stacks its heads at one shared indent. **A
comment on the `=` line, or glued to the first head, does not change that** — the chain
prints exactly as it does with no comment (`a`, the null control), the comment leading the
first head. The template twin of
[curried_head_break_leading_comment](../../../../typescript/expressions/arrow/curried_head_break_leading_comment_prettier_divergence/),
and the head-forced sibling of
[const_curried_chain_long_leading_comment](../const_curried_chain_long_leading_comment_prettier_divergence/),
where the break is forced by width instead: one tag, one answer, whichever forced it.

Prettier's answer changes with the comment's kind, as it does in a `<script>`:

- line comment (`c`) — Prettier **fabricates a blank line** below the comment and indents
  the chain two levels deeper
- single-line block (`d`), a run of them (`e`) — Prettier cancels the break after `=` and
  indents the remaining heads under the first
- indentable block (`f`) — Prettier keeps the break and indents the whole chain one level
  deeper

A preserved multi-line block (`g`) leads the chain from the `=` line — prettier's form, and
the one a width-driven chain takes too — pinned here as the boundary of the rule.

`unformatted_ours_head_on_equals.svelte` is prettier's own output, kept as a normalization
test: a file prettier already formatted converges here in one pass.
`unformatted_ours_paren_shell.svelte` writes every value in a redundant paren shell
(`= /* c */ (({}) => () => b)`), where the comment is owned by the node the strip discards —
the shell strips and changes nothing else, so it reaches the bare authoring's fixed point.

See [conformance_prettier_svelte.md](../../../../../../docs/conformance_prettier_svelte.md) §Svelte: Blocks,
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
