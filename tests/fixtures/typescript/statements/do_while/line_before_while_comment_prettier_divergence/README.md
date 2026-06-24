# line_before_while_comment_prettier_divergence

An own-line line comment between a do-while's `}` and its `while`
(`}⏎// c⏎while (a);`) is preserved on its own line before `while`. Prettier
relocates it inside the while condition parens, breaking the condition.

- Input: `}⏎\t// comment before while⏎\twhile (a);`
- Prettier: `} while (⏎\t// comment before while⏎\ta⏎);` (moved into the parens)
- Ours: keeps the comment on its own line before `while`

Covers both the directly-preceding comment and a blank-line-before-comment case
(`variant_spaces` pins prettier's stable blank-line-inside-parens form).

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of comments before the `while` keyword
([while_leading_block_comment](../while_leading_block_comment_prettier_divergence/))
and around the condition parens
([open_paren_comment](../open_paren_comment_prettier_divergence/),
[close_paren_comment](../close_paren_comment_prettier_divergence/)), and with
if/else, try/catch, switch, for, while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
