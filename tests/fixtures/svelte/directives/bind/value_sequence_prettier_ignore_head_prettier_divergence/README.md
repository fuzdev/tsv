# value_sequence_prettier_ignore_head_prettier_divergence

A **function-binding sequence** (`bind:value={getter, setter}`) frozen whole by an own-line
directive in the `{`→value gap. tsv emits the sequence bare, as it does for every other
function-binding value:

```svelte
<input
	bind:value={
		// prettier-ignore
		()  =>  a,
		(v)  =>  (a  =  v)
	}
/>
```

Prettier parenthesizes it (`(()  =>  a,\n\t\t(v)  =>  (a  =  v))`) on the first pass and then
**drops the directive entirely** on the second, reformatting the value
(`bind:value={() => a, (v) => (a = v)}` — pinned in `audit_signature.txt`). The parens are
also wrong for Svelte: `bind:value={(get, set)}` is a grouped expression, not a getter/setter
pair. So there is no comment-preserving prettier fixed point here, exactly as for the plain
comment case — see the sibling
[function_comment_inline_block](../function_comment_inline_block_prettier_divergence/).

The freeze scope itself matches prettier: a directive in the leading gap leads the
**sequence node**, so every operand rides inside the verbatim slice — see the ordinary
[for-clause](../../../../typescript/statements/for/clauses_prettier_ignore_head/) and
[return/throw](../../../../typescript/statements/return_throw/operand_prettier_ignore_head/)
siblings.

## Reason

User comments are valuable and shouldn't be silently removed, and reproducing prettier's
parenthesized form would both re-introduce the loss and change what Svelte reads. See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).

## Related

- [function_comment](../function_comment/) — the same value's plain leading / mid comments
- [function_comment_inline_block](../function_comment_inline_block_prettier_divergence/) — the same prettier non-idempotency without a directive
- [value_prettier_ignore_head](../value_prettier_ignore_head/) — the ordinary non-sequence head freeze
