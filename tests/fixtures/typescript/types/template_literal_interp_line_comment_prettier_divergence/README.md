# template_literal_interp_line_comment_prettier_divergence

A line comment after a template-literal type's `${` interpolation opener
(`` `a${ // c⏎B}` ``). tsv keeps the comment after `${` and drops the
interpolation type to the next line:

```
type T =
	`a${// c
	B}`;
```

**Prettier** keeps the comment too but fully expands the interpolation, putting
the comment on its own line (`` `a${⏎// c⏎B⏎}` ``) — both preserve the comment;
the divergence is layout.

Per Comment Position Philosophy, tsv preserves the comment's authored position.
Emitting it inline (the previous behavior) let the `//` **swallow** the
interpolation type — non-idempotent content loss; the line comment now forces the
break (the shared `build_trailing_comments_break_for_line`).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
