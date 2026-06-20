# function_comment_inline_block_prettier_divergence

A **single-line block comment leading a function-binding sequence**
(`bind:value={/* c */ getter, setter}`). tsv keeps it bare and idempotent; prettier
is non-idempotent here and ends up dropping the comment.

tsv:

```svelte
<input bind:value={/* c */ a, b} />
```

Prettier: `<input bind:value={/* c */ (a, b)} />` on the first pass, then
`<input bind:value={a, b} />` on the second (the comment is lost). See
`audit_signature.txt` for the pinned chain.

prettier-plugin-svelte prints a bind function-binding sequence with `removeParentheses`,
which strips the sequence's `(…)` **except** when a leading block comment precedes them
(`removeParentheses` stops at the comment, leaving the parens). The resurfaced `(a, b)`
then re-parses as an ordinary parenthesized sequence whose leading comment prettier drops
on the next pass — so there is no comment-preserving fixed point. tsv keeps the sequence
bare and the comment in place, which is lossless and stable.

The line-comment and multi-line block-comment leading cases are **not** divergences — tsv
matches prettier there (preserved in the broken, bare form); see the regular sibling
[function_comment](../function_comment/). Mid (between getter and setter) comments are
also preserved and match prettier (same sibling).

## Reason

User comments are valuable and shouldn't be silently removed; the comment is syntactically
valid here. Reproducing prettier's parenthesized form would re-introduce the loss (and is
non-idempotent), so tsv preserves the comment bare. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [function_comment](../function_comment/) — leading line / multi-line block + mid comments (preserved, match prettier)
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — prettier drops template-expression comments; tsv preserves
