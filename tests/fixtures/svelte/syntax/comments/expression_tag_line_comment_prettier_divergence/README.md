# expression_tag_line_comment_prettier_divergence

A line comment in an expression tag's `{`→value gap. The `//` runs to end of line, so the
value cannot stay on the brace's line, and tsv drops it to a continuation line **indented one
level**; prettier leaves it flush.

```svelte
{// line comment before expression
	a}
```

Prettier: `{// line comment before expression⏎a}`.

The second case is the **control**: a block comment ends with a space rather than a break, so
it forces nothing, the value stays on the brace's line, and both formatters agree. That pins
the trigger as the *break*, not a comment's mere presence.

## Reason

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
— the same rule at every braced head, so `{expr}` cannot answer it differently from
`{@html …}` or `{#if …}`.

## Related

- [expr_leading_line](../expr_leading_line_prettier_divergence/) — the whole braced family swept at once
- [expr_multibyte](../expr_multibyte_prettier_divergence/) — the same pair with multibyte comment text
