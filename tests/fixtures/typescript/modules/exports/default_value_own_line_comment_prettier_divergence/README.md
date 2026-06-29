# default_value_own_line_comment_prettier_divergence

A comment in the `export default`→value gap, with the value authored on a later
line than the comment.

**tsv**: keeps the comment where the author wrote it — an own-line comment
(`export default⏎/* c */⏎x`) stays on its own line — with the value on the next
(indented) line. An author blank line after the comment is preserved:

```
export default
	/* c */

	x;
```

**Prettier**: pulls the comment up onto the `export default` line, value below
(`export default /* c */⏎x`).

Per Comment Position Philosophy: the user put the comment on its own line and the
value below, so tsv preserves that placement rather than relocating the comment
onto the keyword line. This is the value-position counterpart of the
`as`/`satisfies` cast gap
([as_satisfies_value_own_line_block_comment](../../../expressions/as_satisfies_value_own_line_block_comment_prettier_divergence/)).
A same-line comment glued to the value (`export default /* c */ x`) stays inline
in both formatters and is not a divergence. The value's one-level indent is the
uniform module-header indent (see conformance_prettier.md). Emitting the comment
inline (the previous behavior) reflowed the author's break (`export default /* c */ x`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
