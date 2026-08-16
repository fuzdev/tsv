# value_sequence_prettier_ignore_trailing_comment_prettier_divergence

The intersection of the two siblings: a comment after the **last operand** of a `bind:`
function-binding sequence that a `// prettier-ignore` in the `{`→value gap has frozen
whole. The freeze replaces the sequence's own doc with a verbatim slice; it does not own
the gap between that slice's end and the value's `}`, so the comment there is emitted by
the same trailing run the unfrozen sequence uses.

tsv:

```svelte
<input
	bind:value={
		// prettier-ignore
		()  =>  a,
		(v)  =>  (a  =  v) /* c */
	}
/>
```

Prettier: the same block with the comment stripped — and the bare pair re-parenthesized
(`(()  =>  a,` … `(a  =  v))`), which is [value_sequence_prettier_ignore_head](../value_sequence_prettier_ignore_head_prettier_divergence/)'s
pre-existing divergence, independent of the comment.

The third case is the control, and it is what makes the rule legible: a comment written
**inside** the slice needs no emitter at all — it is part of the verbatim text, spacing and
all, and both formatters keep it. Only the position past the slice's end is at stake, which
is why the loss hides: every comment an author is likely to write inside a frozen region
survives, and the one that doesn't looks like it should be inside too.

## Reason

User comments are valuable and shouldn't be silently removed. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [value_sequence_trailing_comment](../value_sequence_trailing_comment_prettier_divergence/) — the same trailing position with no freeze
- [value_sequence_prettier_ignore_head](../value_sequence_prettier_ignore_head_prettier_divergence/) — the same freeze with no comment
- [function_comment](../function_comment/) — the leading and inter-operand positions, where both formatters agree
