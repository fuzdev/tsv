# value_sequence_leading_blank_prettier_divergence

An author blank line after a line comment **leading** a `bind:` function-binding sequence.
The blank survives, as after a leading `//` at every other braced head
([expr_leading_blank](../../../syntax/comments/expr_leading_blank_prettier_divergence/)), and
the sequence stays bare below it ([function_comment](../function_comment/)).

tsv:

```svelte
<input
	bind:value={
		// c

		() => a, (v) => (a = v)
	}
/>
```

Prettier: `// c⏎(() => a, (v) => (a = v)` — the blank dropped and an **unbalanced** paren
opened ahead of the getter, output no parser accepts (◆prettier_bug), so there is no
prettier form of this authoring to converge on. The blank is what trips it: without the blank
prettier leaves the bare sequence alone ([function_comment](../function_comment/)).

## Reason

The blank is the author's, kept at every forced continuation
([conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)),
and the sequence host takes the answer every other head gives. See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [function_comment](../function_comment/) — the leading and inter-operand comment positions without a blank
- [expr_leading_blank](../../../syntax/comments/expr_leading_blank_prettier_divergence/) — the blank after a leading `//` at every braced head, the sequence's comma gap included
