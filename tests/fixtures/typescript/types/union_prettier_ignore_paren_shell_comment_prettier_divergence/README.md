# union_prettier_ignore_paren_shell_comment_prettier_divergence

When a frozen union member is a **redundant** parenthesized type whose paren *shell* holds
a comment (`(/* keep */ a1)`), tsv keeps the WHOLE member verbatim — the redundant paren
included — so the shell comment is preserved. Slicing the inner type to drop the redundant
paren (tsv's normal paren-transparent freeze) would drop the comment, so the freeze keeps
the paren instead: a kept redundant paren under a freeze is correct, a dropped comment
never is.

**Prettier** strips the redundant paren and relocates the comment (`/* keep */ a1`) — it
freezes the first member but still normalizes its parens.

```ts
type A =
	// prettier-ignore
	(/* keep */ a1) | b;   // tsv keeps the paren+comment; prettier → /* keep */ a1
```

`output_prettier.svelte` records prettier's paren-stripped form. The rule holds at both the
first-member position (`type A`) and a between-members position (`type B`).

## Reason

Comment preservation outranks redundant-paren removal under a freeze: the paren-transparent
freeze slices the inner node only when the shell is comment-free, and keeps the whole member
otherwise. See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
