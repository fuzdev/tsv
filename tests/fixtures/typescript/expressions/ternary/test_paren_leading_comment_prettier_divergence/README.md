# Ternary operand leading-comment divergence

A leading block comment glued **inside** a ternary operand's grouping parens
(`(/* c */ x ?? y)`) is preserved where the author wrote it — inside the parens,
before the operand — in the test position and both branches. The parens are
required (`??` can't be mixed with `?:` unparenthesized); both formatters keep
them, only the comment position differs.

tsv: `(/* c */ x ?? y) ? p : q` (preserved)
Prettier: `/* c */ (x ?? y) ? p : q` (relocated out, before `(`)

Both authorings are stable under tsv — the glued-inside form (`input`) and the
leading-paren form (`variant_leading_paren`, which is also Prettier's fixed
point). tsv preserves whichever the author wrote; Prettier collapses the
glued-inside form to the leading-paren one. Every glued block comment is bound
to the operand it leads (`Comment::owned_by_node`), so a paren the printer keeps
around the `??` operand lands outside the comment rather than between the two.

Symmetric with the other operand positions where tsv preserves a leading comment
inside kept parens — the non-null grouped operand
([grouped_operand_leading_comment](../../non_null/grouped_operand_leading_comment_prettier_divergence/))
and the `await`/`yield` grouped operand
([grouped_operand_comment](../../await_yield/grouped_operand_comment_prettier_divergence/)).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Ternary operand leading comment) and §Comment Position Philosophy.
