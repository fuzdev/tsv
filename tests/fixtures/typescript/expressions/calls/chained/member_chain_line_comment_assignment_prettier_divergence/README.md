# Member-only chain with an interior line comment, in assignment-expression / object-property position

The assignment-expression and object-property variant of
[member_only_non_null_line_comment](../member_only_non_null_line_comment_prettier_divergence/)
(itself the non-null variant of
[member_only_interior_line_comment](../member_only_interior_line_comment_prettier_divergence/)):
a member-only chain (pure property access, **no calls**) whose interior line comment
forces the chain to break, on the right-hand side of an **assignment expression**
(`x = foo // c⏎.bar!`) or an **object property** (`k: foo // c⏎.bar!`) rather than a
`const` declaration.

- **tsv**: keeps each comment where the author wrote it and keeps the chain on the
  operator line — the comment-forced break lands at the member, the chain hangs one level
  in (`x = foo // c⏎\t\t.bar!`), never breaking after the `=`/`:`. A trailing non-null `!`
  stays glued to its member (`.bar!` / `?.bar!`).
- **prettier**: breaks after the `=` and hoists the chain onto its own line
  (`x =⏎\t\tfoo // c⏎\t\t.bar!`). (For the object-property value prettier keeps it on the
  `:` line, matching tsv — only the assignment-expression form diverges.)

This is the same comment-preservation choice as the `const`-declaration seed, in the layout
path that keeps the chain glued to the operator (`NeverBreakAfterOperator`); a `const`
declaration prints through a different path (never reaching this one), so the two are
covered separately. tsv keeps comments where authored rather than relocating them across the
assignment boundary; see [comment-position philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

See [conformance_prettier.md §Comment relocation](../../../../../../../docs/conformance_prettier.md#comment-relocation).
