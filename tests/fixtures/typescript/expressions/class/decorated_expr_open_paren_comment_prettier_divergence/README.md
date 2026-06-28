# decorated_expr_open_paren_comment_prettier_divergence

Own-line comment — a **line** comment or an own-line **block** comment — after
the open paren of a **bare parenthesized decorated class expression**
(`(⏎ // c⏎ @decorator⏎ class {}⏎)`) — an open-delimiter `(` trailing-comment case
on the decorated-class-expression path.

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
is the fix. Newly surfaced by decorated-class-**expression** support. (A
*same-line* block comment — `(/* c */ @decorator …)` — is a separate, rarer
inline case still left to the default flow.)

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
