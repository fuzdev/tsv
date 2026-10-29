# Template interpolation, leading comment on an optional chain

A comment leading a `${…}` value occupies the interpolation whatever the value's kind
([interpolation_comment_expression_kinds](../interpolation_comment_expression_kinds/)), so once
the value breaks, the interpolation breaks at its `${` and `}` around it — an optional chain
included, whether the comment is written in the gap (`c1`) or in a stripped shell at the
chain's head (`c2`, `unformatted_ours_hugged`).

- **tsv**: `${⏎→/* c1 */ bbb?.ccc(function () {…})⏎}` — the one rule every value kind takes.
- **prettier**: keeps the `${` hugged (`output_prettier.svelte`, its fixed point). It asks
  `hasComment(node)` of the interpolation's node, and on the estree parsers (`typescript`, which
  prettier-plugin-svelte uses for `lang="ts"`) an optional chain's node is the
  `ChainExpression` wrapper, which never holds a comment (`can-attach-comment.js` excludes it):
  the comment attaches to the call inside, and the check reads the interpolation as
  comment-free. Prettier's own `babel` / `babel-ts` routes have no wrapper and wrap exactly as
  tsv does, as does every parser on the same value without `?.`; the same printer strips the
  wrapper for its member test (`stripChainElementWrappers`) but not for `hasComment`.

See
[conformance_prettier_ts.md §TypeScript: Template Literals](../../../../../../../docs/conformance_prettier_ts.md#typescript-template-literals)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
