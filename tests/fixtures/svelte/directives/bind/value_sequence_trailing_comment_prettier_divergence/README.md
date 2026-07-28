# value_sequence_trailing_comment_prettier_divergence

A comment after the **last operand** of a `bind:` function-binding sequence — the one
position in the sequence prettier deletes. It preserves the leading and inter-operand ones
(the ordinary sibling [function_comment](../function_comment/) pins those, matching), so
this is the trailing-position content loss
[expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) and
[expr_trailing_line](../../../syntax/comments/expr_trailing_line_prettier_divergence/)
already catalog for every other `{…}` value, reaching the sequence host.

tsv:

```svelte
<input bind:value={() => a, (v) => (a = v) /* c */} />
```

Prettier: `<input bind:value={() => a, (v) => (a = v)} />` (comment stripped).

The sequence keeps its own layout around the preserved comment. The operands sit in their
own group, so a trailing comment — which is *outside* it — never breaks them: the pair stays
on one line and only the value's own `{…}` breaks, when the comment is a `//` whose hardline
forces it. The closing `}` then reuses that break rather than adding a second, the rule
[braced_value_trailing_line](../../../syntax/prettier_ignore/braced_value_trailing_line_prettier_divergence/)
states for every unprefixed value.

## Reason

User comments are valuable and shouldn't be silently removed. See
[conformance_prettier.md §Svelte: Attributes](../../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [function_comment](../function_comment/) — the leading and inter-operand positions, where both formatters agree
- [value_sequence_prettier_ignore_head](../value_sequence_prettier_ignore_head_prettier_divergence/) — the sequence's freeze scope
