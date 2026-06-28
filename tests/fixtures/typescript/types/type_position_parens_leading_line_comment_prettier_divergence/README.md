# type_position_parens_leading_line_comment_prettier_divergence

A line comment trailing a single generic type-argument's `<`
(`type C = Array<// leading\n\ta>`).

**tsv** keeps the comment trailing the `<` with the argument on the next line
and `>` glued to it. **Prettier** breaks the type-argument list, dropping the
comment and the argument onto their own lines with `>` on its own line. Both
forms are stable under their respective formatters.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(trailing the `<`) rather than expanding the single-argument list. The fixture's
other cases — a type-parameter constraint (`T extends // leading\n\ta`), a tuple
element, and a function-type return — match both formatters; only the single
generic type-argument case diverges, where prettier expands the `<>`.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation.
