# Sole multiline template argument behind a comment run

Prettier hugs a sole multiline template argument against the `(` unless the author put the
template on a line of its own (`isTemplateOnItsOwnLine`, a newline right before the backtick).
A comment run in the `(`-line gap changes what precedes the backtick in the OUTPUT, which that
source test never reads:

- a run that ends its line — an own-line block, moved in from between the callee and its `(`
  (`c1`, `c6`) or written inside the parens (`c2`, `c3`, and `import()`'s `c7`) — strands the backtick on a line of its own, so the arguments break;
- a block glued to the callee (`c4`) or to a stripped `(` whose newline sits before the
  backtick (`c5`, `import()`'s `c8`, a chain link's `c9`) is glued to the template, which
  hugs the `(`.
- the outer call's answer also decides whether the member chain swallows the call below it
  (`c10`): an outer template on its own line sends the chain through the member-chain
  printer, whose links expand their templates.

- **tsv**: decides the hug by what the chosen layout prints ahead of the backtick, so the
  authored form (`unformatted_ours_relocated`) reaches `input.svelte` in one pass.
- **prettier**: lands on the same form in **two passes** (`prettier_intermediate_relocated.svelte`):
  its first pass hugs `c1`–`c3` / `c6` / `c7` / `c10` behind the comment's line break (and `c10`'s inner link with it) and expands `c5` / `c8` / `c9`, and
  keeps `c4`'s comment ahead of the `(`; the second pass reads its own output and flips each.

Not a placement difference — the fixed point is shared. The `//` spelling is
[template_arg_multiline_shell_line_comment](../template_arg_multiline_shell_line_comment_prettier_divergence/).

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks)
and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
