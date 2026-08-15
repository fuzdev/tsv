# Non-null mid-chain stripped-shell line-comment divergence

The mid-chain cell of
[operand_line_comment](../operand_line_comment_prettier_divergence/): a **line**
comment in the operand→`!` gap of a grouping shell whose `!` continues a chain
(`(aaa // c⏎)!.bbb()`, and the call / computed continuations). tsv retains the
shell and keeps the comment inside on the operand's line — the same multiline
layout the sealed parenthesized base takes
([optional_paren_non_null_sealed_line_comment](../../chain/optional_paren_non_null_sealed_line_comment_prettier_divergence/)).

Prettier strips the parens and relocates the comment after the `!`, breaking
the chain below it (`aaa! // c⏎.bbb();`).

A line comment can't sit in this gap bare — the `!` binds under
[no LineTerminator here], so the `//` would swallow it; the shell is the only
authoring that puts one here, and retaining it is what keeps the comment. A
**block** comment in the same gap needs no shell and matches prettier
(`aaa /* c */!.bbb`, [mid_chain_operand_comment](../mid_chain_operand_comment/)).

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Non-null operand (line comment), the general rule) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
