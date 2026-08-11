# Divergence: declaration head→body `{` own-line block, prettier relocates

An **own-line** single-line block comment in a declaration's head→body-`{` gap
(`function a()⏎/* c */⏎{}`). A single-line block forces nothing, and a comment in this gap
**trails the head** rather than leading the body, so tsv collapses the authored breaks and
keeps it in its authored syntactic slot — `function a() /* c */ {}`, the inline form both
formatters hold stable ([declaration_head_body_comment](../declaration_head_body_comment/)).
Own-line-ness is authoring signal for a *leading* position, not a trailing one.

Prettier instead **relocates** the comment, by two different routes, and reaches its fixed
point in two passes (`prettier_intermediate_to_variant_own_line` → `variant_own_line`,
dual-stable):

- out to between the name and the parameter list for a function declaration and a class
  method (`function a /* c */() {}`) — the destination it also hoists an in-paren comment
  to ([open_paren_block_comment](../../../declarations/function/open_paren_block_comment_prettier_divergence/))
- across the `{` and into the body, leading the first member, for a class and an interface

Both landings are positions from which a reader can no longer tell the comment was written
about the head. The `variant_own_line` form is dual-stable — tsv preserves a comment
authored in either of those slots — so the divergence is entirely in how the
`head⏎/* c */⏎{` authoring **normalizes**.

The other constructs in this gap answer it differently and are pinned separately: a
function expression and an object method, where prettier keeps the break
([break_kept](../declaration_head_body_own_line_block_break_kept_prettier_divergence/)), and
an `enum` / `namespace`, where prettier reaches tsv's own form in two passes
([two_pass](../declaration_head_body_own_line_block_two_pass_prettier_divergence/)).

A **multiline** block the author broke after is outside this rule and keeps its break
([declaration_head_body_multiline_block_break](../declaration_head_body_multiline_block_break_prettier_divergence/),
with its [non-divergent sibling](../declaration_head_body_multiline_block_break/));
a **line** comment forces the break and stays in the gap
([declaration_head_body_line_comment](../declaration_head_body_line_comment_prettier_divergence/)).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
