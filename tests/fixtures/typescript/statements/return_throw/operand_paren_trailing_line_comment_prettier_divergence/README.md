# Return/throw operand paren, trailing line comment

A `//` written between a `return`/`throw` operand and the `)` that closes its
grouping parens forces the parenthesized form on its own — the same layout
[hanging_paren_trailing_comment](../hanging_paren_trailing_comment/) reaches
through a *leading* own-line comment, here with nothing leading it.

- **tsv**: retains the parens and keeps the comment inside them, on its line.
- **prettier**: strips the parens and defers the comment to end of line, past
  the `)` and the `;`.

**Deferring here loses information, which is why tsv diverges rather than
matching.** A `//` carried past its own statement lands on a line that may
already hold one, and the two **merge**: `return (x // c1⏎); // c2` becomes
`return x; // c1 // c2`, which reparses as a *single* comment whose text is
` c1 // c2` — the second comment stops existing (`fn3`). Where the trailing
comment is a block instead, prettier keeps both but **reverses** them: the one
written after the `)` comes out first (`return x; /* t */ // c`, `fn6`). Both
are the information-losing relocation
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
names as its deciding test.

The rule was already tsv's answer at the **binaryish** and **sequence**
operands, whose printers keep such a comment inside their own parens
(`keep_operand_line_inline`); the plain, call and assignment operands were the
holdouts, and one question answered two ways is what this fixture closes. The
same retention holds at the `await`/`yield` operand shell
([redundant_operand_paren_comment](../../../expressions/await_yield/redundant_operand_paren_comment_prettier_divergence/)).

An operand with **no** authored parens is untouched — there is no shell to keep
the comment inside, so the terminator gap is its only home (`return x // c⏎;` →
`return x; // c`, [operand_paren_comment](../operand_paren_comment/)). A
trailing **block** comment is likewise untouched: it does not end its line, so
prettier's placement past the `;` is lossless and stable, and tsv matches it.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Return/throw operand paren, line comment).
