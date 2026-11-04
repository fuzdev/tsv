# Export-default required paren, trailing line comment

A `//` written between the exported value and the `)` of its grouping parens, where the
value is one `export default` wraps in a pair anyway — its leftmost token is `class` or
`function`, so bare it would reparse as a declaration. The authored pair **is** that pair:
tsv prints it once, broken open, with the comment inside it, after the value, where the
author wrote it.

- **tsv**: `export default (⏎\tclass E {} // c1⏎); // c2`
- **prettier**: `export default (class E {}); // c1 // c2` — it prints its own pair flat and
  defers the comment past the `;`, where it **merges** with the trailing `// c2` into one
  comment (`output_prettier.svelte`).

This is the rule of [default_operand_paren_line_comment](../default_operand_paren_line_comment_prettier_divergence/)
at a value that needs its pair: the retained shell discharges the position's pair, so the
value is never paired twice. A decorated class written in the pair takes the same answer
([after_export_default_paren](../../../typescript_specific/decorators/after_export_default_paren_prettier_divergence/)).

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Export value paren, line comment).
