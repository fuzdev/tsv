# Yield hanging-comment paren retention

`yield` is a **restricted production** — the grammar is
`yield [no LineTerminator here] AssignmentExpression` (ECMA-262 §15.5), so a
newline between the keyword and its operand is not layout, it is ASI. When a
comment in that gap forces a break, tsv keeps the grouping parens, which is
what makes the break legal; the comment then stays on the line the author gave
it. A comment that does **not** force a break still collapses inline
(`yield /* c */ x ?? y`) and the redundant parens are dropped as usual.

This is the same rule tsv already applies to `return` and `throw`, the other
two restricted productions — prettier applies it there too
(`printReturnOrThrowArgument`), but its paren-retention check is scoped to
`ReturnStatement`/`ThrowStatement` and never reaches `YieldExpression`.

Prettier therefore drops the parens and emits `yield /* c */⏎x ?? y;`, which
**no longer parses as the input did**: ASI ends the `yield` at the newline, so
the operand becomes a separate expression statement and the `yield` loses its
argument. Prettier's own second pass makes the split explicit —
`yield; /* c */⏎x ?? y;` (see `audit_signature.txt`). The delegate form
`yield*` is affected in layout only: a bare `yield*` is a syntax error, so ASI
cannot silently split it.

Reason: content integrity — a formatting pass must not change what the code
means. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Yield hanging comment) and §Comment Position Philosophy.
