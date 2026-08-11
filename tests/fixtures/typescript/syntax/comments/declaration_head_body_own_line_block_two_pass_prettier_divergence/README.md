# Divergence: `enum` / `namespace` head→body `{` own-line block, two passes

An **own-line** single-line block comment in an `enum` or `namespace` head→body-`{` gap
(`enum A⏎/* c */⏎{ X, Y }`). Both formatters end at the same form — `enum A /* c */ {` —
so this is not a position or layout divergence; it is a **normalization** one. tsv reaches
the fixed point in a single pass, while prettier needs two: its first pass keeps the
author's break with `{` alone on its own line (`prettier_intermediate_own_line`), and its
second glues the brace back.

That first-pass form is a shape prettier produces nowhere else and does not itself hold, so
pinning the chain is what keeps the agreement honest — a one-pass comparison against
prettier reads as a divergence here when there is none.

The sibling constructs in this gap do diverge, and are pinned separately: a function
declaration, class method, class and interface, where prettier **relocates** the comment
([relocated](../declaration_head_body_own_line_block_relocated_prettier_divergence/)), and a
function expression and object method, where it **keeps** the break as its fixed point
([break_kept](../declaration_head_body_own_line_block_break_kept_prettier_divergence/)).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
