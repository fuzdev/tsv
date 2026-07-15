# template_literal_interp_trailing_comment_long_prettier_divergence

The width case of
[template_literal_interp_trailing_comment](../template_literal_interp_trailing_comment_prettier_divergence/):
a comment authored trailing `${` has **already** broken the interpolation, so a too-wide type
simply breaks *inside* the broken form. The width rule adds no second break, and no blank line
separates the comment from the type.

- **tsv** keeps the comment on the `${` line and breaks the union beneath it, one indent level
  in, `}` on its own line.
- **Prettier** emits a **blank line** after `${` and indents the union members an extra level
  (`output_prettier.svelte`).

A hanging comment is the only break mechanism in play here. That is the point of the fixture:
the comment layout and the width layout are alternatives, not things that compose. Stacking
them is what produces the blank line — a comment run ends in the newline that drops the type,
and a width-driven break opens with a newline of its own, so a construct that applies both
emits two.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
