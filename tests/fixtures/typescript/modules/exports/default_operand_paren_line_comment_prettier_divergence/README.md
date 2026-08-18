# Export-default value paren, trailing line comment

A `//` written between the exported value and the `)` that closes its grouping
parens stays **inside** those parens; the shell is retained even though this
position prints no pair of its own.

- **tsv**: retains the parens and keeps the comment inside them, on its line.
- **prettier**: strips the parens and defers the comment to end of line, past
  the `)` and the `;`.

**Deferring here loses information, which is why tsv diverges rather than
matching.** A `//` carried past its own statement lands on a line that may
already hold one, and the two **merge**: `export default (x // c1⏎); // c2`
becomes `export default x; // c1 // c2`, which reparses as a *single* comment
whose text is ` c1 // c2` — the second comment stops existing. That is the
canonical information-losing relocation
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
names as its deciding test, and the comment census measures it directly
(`MISSING "c1"` · `MISSING "c2"` · `EXTRA "c1 // c2"`).

A trailing **block** comment is untouched: it does not end its line, so
prettier's placement past the `;` is lossless and stable and tsv matches it
([default_operand_paren_comment](../default_operand_paren_comment/)). A value
with no authored parens is untouched too — there is no shell to keep the
comment inside.

The same retention holds at the `export =` twin
([export_equals_operand_paren_line_comment](../export_equals_operand_paren_line_comment_prettier_divergence/)),
at the `return`/`throw` operand
([operand_paren_trailing_line_comment](../../../statements/return_throw/operand_paren_trailing_line_comment_prettier_divergence/))
and at the `await`/`yield` operand shell
([redundant_operand_paren_comment](../../../expressions/await_yield/redundant_operand_paren_comment_prettier_divergence/)).

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Export value paren, line comment).
