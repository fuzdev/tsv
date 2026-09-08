# union_single_member_head_comment_prettier_divergence

An **own-line block comment after a sole union member's authored pipe** — the
`|⏎/* c */⏎{ a: number }` spelling at every value seam: `:` (variable, property
signature), `=>`, `as`, a conditional's `extends` and its branches, a type parameter's
`extends`, a predicate's `is`, `keyof (`, a mapped `in`, an indexed access's `[`, a
comment-free shell around the composite, and `=`. A one-member union prints transparently
as its member (prettier drops the node in postprocess), so the comment sits in the
**seam's own gap** and takes the seam's bare answer: a single-line block in any position
collapses inline (`let a: /* c */ { a: number }`, the keyword→value rule and the
annotation's [annotation_leading_block](../comments/annotation_leading_block_prettier_divergence/)
normalization) at every seam but `=`, where an own-line run keeps its line
(`type E =⏎/* c */⏎{ a: number }`, the control at the end). One pass, and the same fixed
point the pipe-less authoring reaches.

- `unformatted_ours_own_line_pipe.svelte` — the pipe authoring at every seam; tsv
  normalizes it to `input.svelte`.
- `prettier_intermediate_to_variant_own_line_pipe.svelte` — prettier's unstable first pass:
  the pipe dropped, the comment pulled onto the operator's line with the author's break
  kept (`let a: /* c */⏎{ a: number }`).
- `variant_own_line_pipe.svelte` — where that lands, dual-stable: the `:`, `=>`, `keyof`,
  branch and `=` cells reach input's form, while the relocation gaps settle on prettier's
  cataloged before-keyword forms (`x /* c */ as {…}`, `T /* c */ extends`,
  `x /* c */ is`, `P /* c */ in A`, `p /* c */:`, `X /* c */[K]`), which tsv keeps as
  authored.

## Reason

The divergence is prettier's pass count and its relocations, both cataloged where the
bare authorings are. The claim tsv makes here is **transparency**: a sole-member union's
head gap is the enclosing seam's gap, answered by that seam's rule and nothing else — the
union node has no `|` to print, so any layout it chose for itself (a hang with the run
own-line) would be read back next pass through the bare member and re-laid, two forms for
one program. The line-comment sibling is
[union_single_member_head_line_comment](../union_single_member_head_line_comment_prettier_divergence/).

See [conformance_prettier.md §Authored breaks in value position](../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
