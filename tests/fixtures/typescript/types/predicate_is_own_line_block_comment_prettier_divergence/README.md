# predicate_is_own_line_block_comment_prettier_divergence

An **own-line block** comment after a type predicate's `is`, before the predicate
type (`function f(x): x is⏎/* c */⏎T`).

**tsv** keeps the comment on its own line after `is` and hangs the predicate type
on the next line:

```
function f(x): x is
	/* c */
	T {
```

**Prettier** pulls the comment up onto the `is` line, then on a second pass
relocates it *before* `is` and collapses (`x /* c */ is T`) — non-idempotent, so
`audit_signature.txt` pins the chain.

A **same-line** block comment trailing `is` with the type below (`x is /* c */⏎T`,
no preceding newline) likewise keeps the comment trailing `is` and the type
hanging; prettier relocates it before `is` and collapses. A block comment **glued**
to the type (`x is /* c */ T`) stays inline in both formatters and is not a
divergence.

This is the own-line-block sibling of the
[line-comment form](../predicate_is_line_comment_prettier_divergence/) (there
prettier floats the comment to the body `{`); both share the keyword→value hang
(`append_keyword_value_line_comments`). Per Comment Position Philosophy, tsv keeps
the comment associated with the predicate where the author wrote it. A same-line
block comment glued to the type (`x is /* c */ T`) stays inline in both formatters
and is not a divergence.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
