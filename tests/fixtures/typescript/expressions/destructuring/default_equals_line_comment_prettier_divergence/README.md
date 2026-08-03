# default_equals_line_comment

A **line** comment between a binding name/key and its `=` default
(`p // c⏎= v`) — at a parameter default and every destructuring default
(object shorthand, object non-shorthand, array element, function-param
destructure).

- **tsv**: keeps the comment trailing the name, with `= value` broken to the
  next line and indented one level (the uniform forced-continuation indent);
  the pattern expands. Preserves the authored position, and stops the `//`
  from swallowing the `=` and default value.
- **prettier**: relocates the comment to trail the whole binding after the
  value (`{ a = 1 // c }`).

The last case is a **run** whose final comment is a block: the `//` is what
forces the break, so the block and the `=` both land on the continuation line
rather than flush at the binding's own indent. Prettier splits the run across
two relocations — the block hoisted to lead the binding, the line comment
floated past the value — reordering the two.

This is the *before*-`=` counterpart of `param_default_line_comment` (the
after-`=` case). The block-comment sibling `default_equals_comment` stays
inline in both formatters (not a divergence); a **multiline** block the author
broke after keeps its break, at
[default_equals_multiline_block_break](../default_equals_multiline_block_break_prettier_divergence/).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
