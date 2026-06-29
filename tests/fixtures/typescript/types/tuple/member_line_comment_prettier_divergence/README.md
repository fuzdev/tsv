# member_line_comment_prettier_divergence

A line comment after a tuple rest `...` (`[... // c⏎U]`), a named member's `:`
(`[x: // c⏎U]`), or an optional member's `?:` (`[x?: // c⏎U]`). tsv keeps the
comment where the author wrote it and drops the element type to the next line:

```
type N = [
	x: // c
	U
];
```

**Prettier** relocates a named-member comment to *after* the element type
(`[x: U // c]`); it keeps the rest `...` comment in place (`...// c⏎U`).

Per Comment Position Philosophy, tsv preserves the comment's authored position.
Emitting it inline (the previous behavior) let the `//` **swallow** the element
type — non-idempotent content loss; the line comment now forces the break (the
shared `build_trailing_comments_break_for_line` / `build_leading_comments_break_for_line`).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
