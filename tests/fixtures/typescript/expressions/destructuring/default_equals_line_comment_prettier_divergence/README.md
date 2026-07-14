# default_equals_line_comment

A **line** comment between a binding name/key and its `=` default
(`p // c⏎= v`) — at a parameter default and every destructuring default
(object shorthand, array element, function-param destructure).

- **tsv**: keeps the comment trailing the name, with `= value` broken to the
  next line; the pattern expands. Preserves the authored position, and stops
  the `//` from swallowing the `=` and default value.
- **prettier**: relocates the comment to trail the whole binding after the
  value (`{ a = 1 // c }`).

This is the *before*-`=` counterpart of `param_default_line_comment` (the
after-`=` case). The block-comment sibling `default_equals_comment` stays
inline in both formatters (not a divergence).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
