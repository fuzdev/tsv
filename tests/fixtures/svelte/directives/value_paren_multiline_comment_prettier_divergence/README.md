# value_paren_multiline_comment_prettier_divergence

A **multi-line block comment leading a parenthesized directive-value expression**
(`bind:value={/* c⏎*/ (a > b)}`, `class:active={/* c⏎*/ (a > b)}`). The grouping
parens are redundant, so both formatters strip them — but tsv keeps the comment and
expands to the multi-line form (its stable, idempotent shape), while prettier **drops
the comment** as it strips the parens.

tsv (the `input.svelte` fixed point):

```svelte
<input
	bind:value={
		/* c
		 */ a > b
	}
/>
```

Prettier: `<input bind:value={a > b} />` — the leading comment is lost with the parens.

The `unformatted_ours_paren.svelte` variant is the compact paren authoring tsv
normalizes to `input.svelte`; prettier does not (it drops the comment), so it carries
the divergence. `variant_comment_dropped.svelte` pins prettier's actual endpoint —
`{a > b}` with the comment gone — which is dual-stable (both formatters keep it), so
tsv cannot recover the lost comment once prettier has run: preserving it up front is
the only lossless path.

The **bare** authoring (no parens, `bind:value={/* c⏎*/ a > b}`) is **not** a
divergence — there the comment is glued to its operand and both formatters preserve it
in the same expanded form, which is exactly the `input.svelte` shape. Only the
paren-stripping path diverges: prettier discards the comment along with the redundant
parens, where tsv preserves it (a multi-line block comment then forces the same break
the bare form takes). This is the multi-line, directive-value sibling of the
single-line function-binding case
[function_comment_inline_block](../bind/function_comment_inline_block_prettier_divergence/),
where prettier likewise loses a leading comment around parens.

## Reason

User comments are valuable and shouldn't be silently removed; the comment is
syntactically valid here, and reproducing prettier's paren-stripped form would drop it.
tsv preserves the comment and reaches its stable expanded form on the first pass. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
