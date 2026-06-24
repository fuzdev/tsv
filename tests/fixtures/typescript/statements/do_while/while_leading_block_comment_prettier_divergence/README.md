# while_leading_block_comment_prettier_divergence

An own-line block comment leading the `while` keyword (`}⏎/* c */⏎while (cond);`)
is preserved on its own line before `while`. Prettier moves it inside the
condition parentheses, breaking the condition.

tsv: preserves comments before the `while` keyword
Prettier: relocates comments inside the while condition

The divergence is specific to the **own-line** form. When the block comment is
inline on the `while` line (`} while (/* c */ cond);`), both formatters keep it
inside the parens — that form is dual-stable (`variant_leading`, identical to
both formatters' output).

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling of own-line comments before `while` ([line_before_while_comment](../line_before_while_comment_prettier_divergence/)) and other control flow statements.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
