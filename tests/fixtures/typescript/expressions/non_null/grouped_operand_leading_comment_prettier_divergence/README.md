# Non-null grouped-operand leading-comment divergence

The leading-comment counterpart of
[grouped_operand_comment](../grouped_operand_comment_prettier_divergence/) (the
trailing case). When a parenthesized operand of a non-null assertion carries a
**leading** comment inside its required parens (`(/* b */ x + y)!`), tsv keeps
the comment where the author wrote it — inside the parens, before the operand.
The parens are required (`!` binds tighter than `+`/`? :`); both formatters keep
them, only the comment position differs.

Prettier 3.9 relocates the comment **outside** the parens, before `(`
(`/* b */ (x + y)!`).

This is symmetric with the trailing case: tsv preserves an operand comment
inside the kept parens whether it leads or trails. When the parens are redundant
and stripped (a simple identifier `(/* b */ x)!`, a member `(/* b */ a.b)!`), the
comment lands before the operand in both formatters — a match, not a divergence.

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Non-null grouped operand) and §Comment Position Philosophy.
