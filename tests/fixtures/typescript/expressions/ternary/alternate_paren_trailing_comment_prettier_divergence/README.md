# alternate_paren_trailing_comment_prettier_divergence

A trailing comment in the gap of a ternary **alternate**'s grouping shell — the one
branch position the statement's `;` follows (`const a = cond ? 0 : (b = c // c1);`).

**tsv** keeps the comment inside the branch's pair — a single pair, whether it comes
from the retained shell (a line comment) or from the value's own required parens (a
sequence). **Prettier** strips the shell and floats the comment out past the `;`.

## Reason

The alternate is the only branch the terminator is adjacent to, so it is the only one
where the deferral is even reachable — a consequent's gap ends at the `:`. But the pair
is still **in the output**: an assignment, `??` and `as` branch each take clarity parens
([branch_comment_paren](../branch_comment_paren/)), and a sequence supplies its own. The
deferral's licence is *"this output erases that `)`, so the next pass reads the comment
as statement-trailing"* — false wherever the branch prints a pair, so the comment stays
inside it rather than crossing a `)` the reader can still see.

That is the same answer the statement-level value position gives
([init_assignment_paren_line_comment](../../../statements/variable/init_assignment_paren_line_comment_prettier_divergence/),
`const y = (a = b // c)`) and the same one a `for`-header init declarator gives
([init_paren_line_comment](../../../statements/for/init_paren_line_comment_prettier_divergence/)).
Prettier is internally inconsistent about it: at a declarator initializer it keeps a
sequence's trailing block inside the surviving pair
([value_position_trailing_comment](../../sequence/value_position_trailing_comment/),
`const x = (a, b /* c */)`), and here — the same pair, the same comment — it floats it
past the `;`.

Prettier's form is not a fixed point: a second pass collapses the ternary and, for a
**run**, carries the later comment out to the enclosing statement list. Both are pinned
in `audit_signature.txt`.

The **block**-comment case on a non-self-parenthesizing branch is not a divergence — the
shell strips and the comment defers past the `;` in both formatters, since nothing is
left to hold it (`const a = cond ? 0 : (b /* c */);` → `const a = cond ? 0 : b; /* c */`),
which is the `;`-terminated value-position rule
([value_paren_trailing_block_comment](../../../syntax/comments/value_paren_trailing_block_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
