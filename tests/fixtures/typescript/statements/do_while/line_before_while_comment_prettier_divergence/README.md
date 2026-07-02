# line_before_while_comment_prettier_divergence

An own-line line comment between a do-while's `}` and its `while`
(`}\n// c\nwhile (a);`) is kept on its own line before `while`. Prettier
relocates it inside the `while` condition parens, breaking the condition across
lines.

Covers both the directly-preceding comment and a blank-line-before-comment case.
`variant_comment_before_while.svelte` pins the directly-preceding form, which is
dual-stable (both formatters keep it). `divergent_variant_spaces.svelte` pins prettier's
stable blank-line-inside-parens form: prettier keeps the blank, but tsv drops it
and settles on a third stable form (distinct from both prettier's and the input) —
a divergent variant.

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of comments before the `while` keyword
([while_leading_block_comment](../while_leading_block_comment_prettier_divergence/))
and around the condition parens
([open_paren_comment](../open_paren_comment_prettier_divergence/)), and with
if/else, try/catch, switch, for, while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
