# init_assignment_paren_line_comment_prettier_divergence

A line comment trailing a **parenthesized assignment** operand in a value position
(here a variable initializer, `const y = (a = b // c)`).

**tsv** keeps the comment inside the parens, trailing the operand, which breaks the
parens across lines. **Prettier** floats the comment out past the `;`. Both forms are
stable under their respective formatters.

## Reason

tsv preserves the author's placement — the comment is inside the parens, trailing the
operand — consistent with how it keeps the trailing comment inside for the block-comment
case (`const w = (a = b /* c */)`), the sequence cases (`const x = (a, b // c)`), and the
return value position
([value_position_trailing_comment](../../../expressions/sequence/value_position_trailing_comment/)).
Prettier is internally inconsistent here: it keeps the *block* comment inside but
relocates the *line* comment out past the `;`. Per Comment Position Philosophy, tsv
treats the authored position as intentional and does not relocate the comment across the
`)`/`;`.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
