# type_argument_paren_union_leading_line_comment_prettier_divergence

A line comment leading a parenthesized union / intersection type argument
(`type A1 = Foo<(// c⏎a | b)>`). The parens are redundant in type-argument
position, so both formatters strip them — landing the comment trailing the `<`.

**tsv** keeps the comment on the `<` line and expands the list below it, the
argument indented one level with `>` on its own line. **Prettier** drops the
comment onto its own line inside the expanded list. Both forms are stable under
their respective formatters.

This is the N=1 form of the multi-argument `<`-trailing divergence
([type_args_open_angle_comment](../../expressions/calls/type_args_open_angle_comment_prettier_divergence/)),
not a shape of its own — the single-argument case takes the same delimiter-line
placement, indented body, and dangling `>` the list already takes, so the
argument count changes nothing.

The multi-argument case (`A3`) renders identically — the comment is on the `<`
line either way, so the argument count changes nothing, which is the point.
The nested case (`A4`) shows the forced break propagating — the enclosing
intersection breaks in both formatters. The call case (`x`) is the same
divergence through the call/`new` instantiation builder. The run case (`A5`)
bounds the delimiter-line rule: only the comment actually on the `<` line stays
there; a later one leads the argument inside the list.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(trailing the `<` once the redundant parens are stripped) rather than relocating
it to its own line.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
