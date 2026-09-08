# union_single_member_head_line_comment_prettier_divergence

A **line comment after a sole union member's authored pipe** — `| // c⏎A` — at every
value seam: `:` (variable, property signature), `=>`, `as`, a conditional's `extends` and
its branches, a type parameter's `=`, a predicate's `is`, `keyof (`, a mapped `in`, an
indexed access's `[`, a type alias's `=`, a type argument's `<`. A one-member union prints
transparently as its member (prettier drops the node in postprocess), so the `//` is the
**enclosing seam's** and takes that seam's own line-comment answer — the uniform
forced-continuation indent (`let a: // c⏎\t\t{ a: number }`), the delimiter-line form at
`[` and `<` — exactly the form the pipe-less authoring lands on. One pass; the pipe never
survives.

- `unformatted_ours_pipe.svelte` — the pipe authoring at every seam; tsv normalizes it to
  `input.svelte`.
- `output_prettier.svelte` — prettier's form from `input`: the continuation flush at the
  statement's indent where it keeps the comment in the gap, and its cataloged relocations
  elsewhere (past the statement at `as` / `is` / `[` / a property signature, trailing the
  check type at `extends`, trailing the constraint at `in`).
- `prettier_intermediate_to_divergent_variant_pipe.svelte` — prettier's first pass from the
  pipe authoring, which keeps the `is` and `?` comments in their gaps for one pass.
- `divergent_variant_pipe.svelte` — where that chain lands: a prettier fixed point that
  differs from `output_prettier` at the `?` branch (the comment trails the check type) and
  that tsv rewrites to a third form, re-indenting the flush continuations.

## Reason

Transparency: the union node has no `|` to print, so a layout it chose for its own head
gap (the leading-pipe layout, `:⏎| // c⏎  {…}`) was read back on the next pass through the
bare member and re-laid by the seam — two fixed points for one program. The seam's answer
is the one already cataloged for the pipe-less spelling at each site
([§Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent));
the block-comment sibling is
[union_single_member_head_comment](../union_single_member_head_comment_prettier_divergence/).

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
