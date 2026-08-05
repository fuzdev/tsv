# type_position_parens_leading_line_comment_prettier_divergence

A line comment leading a single-slot type position — a generic type-argument's
`<` (`type C = Array< // leading\n\ta\n>`) and a function type's `=>`
(`type D = () => // leading\n\ta`).

Two divergences, one for each:

- **Type argument (`C`).** tsv keeps the comment on the `<` line and expands the
  list below it, the argument indented one level with `>` on its own line — the
  N=1 form of the multi-argument `<`-trailing divergence. Prettier drops the
  comment onto its own line inside the expanded list.
- **Function-type return (`D`).** Both keep the comment trailing the `=>`; tsv
  indents the return type one level (§Uniform Forced-Continuation Indent, the
  layout every other keyword→value gap takes — including this fixture's own
  type-parameter constraint case, and the frozen `=>` gap in
  [function_type_prettier_ignore_return](../function_type_prettier_ignore_return_prettier_divergence/)),
  where prettier leaves it flush.

Both forms are stable under their respective formatters.

## Reason

Per Comment Position Philosophy, tsv keeps each comment where the author wrote it
rather than relocating it; what differs is the layout of what follows. For the
type argument the list expands exactly as prettier's does, the comment simply
staying on the `<` line. For the return type the comment does not move at all —
only the continuation indent differs, which is the uniform rule rather than a
per-construct choice. The remaining cases — a tuple element and the
type-parameter constraint — match both formatters.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
