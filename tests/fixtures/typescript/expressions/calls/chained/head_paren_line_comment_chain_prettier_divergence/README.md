# head_paren_line_comment_chain_prettier_divergence

A `//` the author wrote inside stripped grouping parens at a member chain's HEAD
(`unformatted_ours_paren_head.svelte`). Both formatters strip the parens and print the
comment above the chain, and both settle on the same **flat** chain — `input.svelte` is
a fixed point for each. They differ in how many passes it takes.

- **tsv** reaches it in one pass. The head's comment run is printed ahead of the whole
  chain doc, so the chain's own layout never sees it — which is the same layout the
  identical document gets when the author leaves the parens out (`// c⏎a.b.c(x).d(y)`,
  a plain match on both sides). Erasing the author's redundant parens is no reason to
  change the chain's layout.
- **Prettier** is **non-idempotent** on the paren authoring. It binds the comment to the
  base node, so the hardline it forces lands inside the chain's first group and
  `printMemberChain`'s "any group but the last one has a hard line" rule expands the
  whole chain (`a.b⏎↹.c(x)⏎↹.d(y)`). Its second pass, with the parens already gone, has
  no comment inside group 0 any more and collapses that back to flat.
  `prettier_intermediate_paren_head.svelte` pins the unstable first pass.

Both spellings of the same head are covered: one stripped paren (`c1`) and two nested
(`c2`). They agree here — the layout follows the document, not the paren count.

The divergence needs a chain past prettier's group `cutoff`. A shorter one
(`(// c⏎a).b.c(x)`) is flat in one pass on both sides, and is pinned — with the claim
partition the nested spelling turns on — in
[nested_stripped_paren_head_comment](../nested_stripped_paren_head_comment/).

See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
