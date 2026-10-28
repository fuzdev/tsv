# head_paren_line_comment_tag_base_prettier_divergence

The [head_paren_line_comment_chain](../head_paren_line_comment_chain_prettier_divergence/)
rule one node deeper: the stripped grouping parens holding the comment sit around the tag of
a **tagged template** that is the chain's base (`(// c⏎a)`t`.g().h()`), or below that tag
(`((// c⏎a).b)`t`…`), so the comment is not in the chain's own head window. The tagged
template hoists it ahead of its own doc, and that doc is the chain's first group.

- **tsv** reaches the settled form in one pass. The chain head claims the run its base's
  doc opens with on a line of its own and prints it ahead of the whole chain doc, so the
  chain's layout never sees the line break — the layout the identical document gets when the
  author leaves the parens out (`// c⏎a`t`.g().h()`, a plain match on both sides). A `//` and
  a block the author gave a line of its own (`c4`) alike, at a statement, an assignment, an
  argument, `return` (`c5`) and an arrow body (`c6`).
- **Prettier** is **non-idempotent** on the paren authoring: the comment's line break lands
  inside the chain's first group and expands the whole chain (`a`t`⏎↹.g()⏎↹.h()`); its
  second pass collapses it back to flat (`audit_signature_tag_head.txt`). At the assignment
  (`c3`) that second pass also takes its operator-line landing for a left-side shell run
  (`x = // c3⏎↹a.b`t`.g().h();`, the
  [rhs_left_side_shell_comment](../../../assignment/rhs_left_side_shell_comment_prettier_divergence/)
  divergence).

`unformatted_ours_tag_head.svelte` carries the authoring.

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
