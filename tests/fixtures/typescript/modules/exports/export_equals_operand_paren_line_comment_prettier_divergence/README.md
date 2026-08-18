# Export-assignment value paren, trailing line comment

The `export =` twin of
[default_operand_paren_line_comment](../default_operand_paren_line_comment_prettier_divergence/):
a `//` written between the exported value and the `)` that closes its grouping
parens stays **inside** those parens.

- **tsv**: retains the parens and keeps the comment inside them, on its line.
- **prettier**: strips the parens and defers the comment to end of line, past
  the `)` and the `;`, where it **merges** with whatever already trails that
  line (`export = (x // c1⏎); // c2` → `export = x; // c1 // c2`, one comment
  where there were two).

The two export forms share a value position and must answer it alike; a
trailing **block** comment is untouched at both (it does not end its line, so
its placement is lossless and stable).

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Export value paren, line comment) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
§Comment Position Philosophy.
