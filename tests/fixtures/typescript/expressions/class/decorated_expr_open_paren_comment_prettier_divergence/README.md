# decorated_expr_open_paren_comment_prettier_divergence

Own-line comment — a **line** comment or an own-line **block** comment — after
the open paren of a **bare parenthesized decorated class expression**
(`(⏎ // c⏎ @decorator⏎ class {}⏎)`) — an open-delimiter `(` trailing-comment case
on the decorated-class-expression path. The decorated-class form of the
divergence every required-paren expression statement shares
([expression_statement_paren_kept_comment](../../../statements/expression_statement_paren_kept_comment_prettier_divergence/)).

**Prettier** relocates the comment out of the parens onto its own line before
`(`:

```
// c
(
	@decorator
	class {}
);
```

**tsv** keeps the comment where the user wrote it — inside the parens, after
`(`:

```
(
	// c
	@decorator
	class {}
);
```

Per Comment Position Philosophy: the comment sits after the opening `(`, so tsv
keeps it there rather than hoisting it before the parens. tsv previously
**dropped** this comment (content loss — the bare parenthesized decorated class
expression collapsed inline, where an own-line comment cannot go); preserving it
is the fix. Newly surfaced by decorated-class-**expression** support. A block
glued to the `(` (`(/* c */⏎ @decorator …`) takes the same own-line place inside
the broken parens; one glued to the *decorator* is owned by it and stays inline.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
