# sloppy_with_statement_paren_glued_line_comment_prettier_divergence

A **line** comment the author wrote on a `with` header's `(` line (`with ( // c`). tsv
keeps it there; **prettier moves it to its own line** inside the parens.

```
// tsv                          // prettier
with ( // glued                 with (
	scope                               // glued
) {                               scope
	fn();                         ) {
}                                 fn();
                                }
```

This is the `with` spelling of
[condition_paren_glued_line_comment](../../syntax/comments/condition_paren_glued_line_comment_prettier_divergence/),
which carries the argument (the opening-delimiter rule, and prettier's three different
answers across one statement family) for `if` / `while` / do-while / `switch` / `catch`.
`with` shares `while`'s header builder and answers the same way, but cannot sit in that
`.svelte` fixture: a `with` statement parses only in a sloppy script, and a Svelte
`<script>` is a module. The block-comment bound is the same too — `with (/* c */ scope)`
collapses inline in both formatters.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
