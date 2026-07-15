# Sequence operand edge comment divergence

A comment glued to the **leading** edge of a redundantly-parenthesized
sequence-expression operand — before the first operand, inside the sequence
parens (`fn((/* c */ x, y))`) — is preserved where the author wrote it. Prettier
relocates it out of the sequence parens, before `(`:

- tsv: `fn((/* c */ x, y))` (preserved)
- Prettier: `fn(/* c */ (x, y))` (relocated out)

Both authorings are stable under tsv — the glued-inside form (`input`) and the
outside form (`variant_leading_paren`, also Prettier's fixed point). Every glued
block comment binds to the operand it leads (`Comment::owned_by_node`), so the
comment stays inside the parens rather than hoisting across the boundary.

The **trailing** edge is not a divergence: a comment on the last operand's outer
edge floats after the sequence parens (`fn((x, y) /* d */)`) in **both**
formatters, so that line reads the same in `input` and `output_prettier`. The
both-edges case combines the two — the leading comment diverges, the trailing one
matches. Interior operand comments (between two operands) also match prettier —
see the regular fixture `sequence/operand_comments`.

Symmetric with the other operand positions where tsv preserves a leading comment
inside kept parens: the ternary operand
([test_paren_leading_comment](../../ternary/test_paren_leading_comment_prettier_divergence/)),
the non-null grouped operand
([grouped_operand_leading_comment](../../non_null/grouped_operand_leading_comment_prettier_divergence/)),
and the `await`/`yield` grouped operand
([grouped_operand_comment](../../await_yield/grouped_operand_comment_prettier_divergence/)).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Sequence operand leading comment) and §Comment Position Philosophy.
