# line_before_while_comment_prettier_divergence

An own-line line comment between a do-while's `}` and its `while`
(`}\n// c\nwhile (a);`) is kept on its own line before `while`. Prettier
relocates it inside the `while` condition parens, breaking the condition across
lines.

A blank line the author leaves *above* the comment is preserved — the `}`→`while`
gap has no body `{` to sit below it, so the blank separates the block from the
continuation keyword and is authoring intent (see §"No blank above a body block's
`{`" in the conformance doc). Prettier is no oracle for it here: it relocates the
comment into the condition parens and the blank goes with it.
`variant_comment_before_while.svelte` pins prettier's relocated form (comment
inside the condition parens), which is dual-stable. `divergent_variant_spaces.svelte`
pins prettier's stable blank-line-*inside*-parens form: prettier keeps *that* blank,
but tsv drops it and settles on a third stable form (distinct from both prettier's
and the input) — a divergent variant.

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling
of comments before the `while` keyword
([while_leading_block_comment](../while_leading_block_comment_prettier_divergence/))
and around the condition parens
([open_paren_comment](../open_paren_comment_prettier_divergence/)), and with
if/else, try/catch, switch, for, while, labeled statements, and call chains.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
