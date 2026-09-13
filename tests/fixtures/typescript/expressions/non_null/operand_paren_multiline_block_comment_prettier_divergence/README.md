# operand_paren_multiline_block_comment_prettier_divergence

A **multiline** block comment between a non-null assertion's operand and its `!`
(`(x /* m1⏎m2 */)!`).

**tsv**: keeps the grouping parens that hold the comment, and opens them, the form a
`//` in the same gap already takes:

```
let a = (
	x /* m1
m2 */
)!;
```

**Prettier**: strips the parens and inlines the comment:

```
let a = x /* m1
m2 */!;
```

## Reason

The non-null `!` is `[no LineTerminator here]` (TypeScript's `parsePostfixExpressionRest`
stops at `scanner.hasPrecedingLineBreak()`), and the comment's interior newline is a line
terminator. Prettier's form therefore **does not parse** — `x /* m1⏎m2 */` is a
statement and the `!` that follows it is a syntax error — which is why the shell is
load-bearing: the comment can only ever have reached this gap from inside a grouping
pair, and the pair has to survive for the output to mean what the input did. The same
rule the [postfix `++`/`--`](../../unary/update_postfix_paren_line_comment_prettier_divergence/)
and [`as`/`satisfies`](../../as_satisfies_operand_line_comment_prettier_divergence/)
operands already record, and a prettier bug at every one of them (see
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index)).

The retained shell **expands** rather than gluing the operand to the `(`, so a `//` and a
multiline block reach one form at this gap instead of two.

One rule across every spelling of the gap: a plain operand, a single-line block ahead of
the multiline one (both ride the shell), a member or call access continuing the chain
past the `!` (the chain keeps the shell as a parenthesized base), a bare optional chain
(whose parens tsv otherwise strips as redundant —
[optional_paren_non_null_bare](../../chain/optional_paren_non_null_bare_prettier_divergence/);
prettier keeps them there and inlines the comment inside, an output that happens to
parse but sits flat where every other retained shell opens), and an operand that needed
the parens anyway, which takes no second pair.

A **single-line** block comment forces nothing and stays inline without parens
(`k /* c2 */!`, matching prettier) — pinned as the control, alongside the regular
[operand_paren_comment](../operand_paren_comment/) and
[mid_chain_operand_comment](../mid_chain_operand_comment/) fixtures.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
