# default_value_same_line_comment_prettier_divergence

A **same-line** comment after `export default` (on the keyword's line), with the
value authored on the next line (`export default /* c */⏎x`).

**tsv** keeps the comment trailing the keyword and the value on its own line,
indented one level:

```
export default /* c */
	x;
```

**Prettier** also keeps the comment trailing the keyword and the value on its own
line, but at the statement's own indent (no extra level):

```
export default /* c */
x;
```

So the **comment position matches** — both trail `export default`; the only
difference is the value's indent. tsv indents every module-header continuation
one level (uniform with the other `export`/`export default` gaps — see
conformance_prettier.md), so this is an **indent-only** divergence.

The split is intentional: an **own-line** comment is a *position* divergence
(prettier pulls it onto the keyword line) — that case lives in the sibling
[default_value_own_line_comment](../default_value_own_line_comment_prettier_divergence/)
(a module allows only one `export default`, so each case needs its own fixture).
A value **glued** to the comment (`export default /* c */ x`) stays fully inline
in both formatters. Emitting the value inline when it was authored on the next
line (the previous behavior) reflowed the author's break.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
