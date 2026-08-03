# line_comment_prettier_divergence

A line comment in an `on:` directive value's `{`→value gap, over a **self-expanding** value
(an arrow with a block body). The value's own braces make it a hugged one, so nothing else
indents the content — which is what makes this the arm that has to supply the continuation
indent itself:

```svelte
<button
	on:click={// comment
		() => {
			fn();
		}}
>
```

Prettier leaves the arrow flush with the attribute (`on:click={// comment⏎() => {`).

The second case is the **control**: a block comment forces no break, so the value stays on the
brace's line and both formatters agree.

A directive value whose expression does *not* self-expand takes block structure instead
(`use:fn={⏎\t// c⏎\texpr⏎}`) — the comment lands on its own line and the block's own `indent`
is the hang, so the two shapes differ in delimiter treatment only, never in whether the value
hangs.

## Reason

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).

## Related

- [expr_leading_line](../../../syntax/comments/expr_leading_line_prettier_divergence/) — the whole braced family swept at once, including the block-structure directives
- [expr_line_comment](../../../syntax/comments/expr_line_comment/) — line comments *inside* an arrow body, which this rule does not reach
